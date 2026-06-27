use anyhow::anyhow;
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct DescribeReq {
    pub media_id: String,
    #[serde(default)]
    pub aspects: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct DescribeResp {
    pub prompt: String,
    /// false when the vision model was unavailable and a placeholder was returned.
    pub described: bool,
}

const ALL_ASPECTS: [&str; 5] = ["face", "body", "posture", "clothing", "environment"];

/// `POST /api/describe` — look at an uploaded image and produce a detailed diffusion
/// prompt covering the selected aspects (face/body/posture/clothing/environment).
pub async fn describe(
    State(st): State<AppState>,
    Json(req): Json<DescribeReq>,
) -> AppResult<Json<DescribeResp>> {
    // Default to all aspects; keep only known ones, preserving canonical order.
    let aspects: Vec<String> = if req.aspects.is_empty() {
        ALL_ASPECTS.iter().map(|s| s.to_string()).collect()
    } else {
        ALL_ASPECTS
            .iter()
            .filter(|a| req.aspects.iter().any(|r| r.eq_ignore_ascii_case(a)))
            .map(|s| s.to_string())
            .collect()
    };
    if aspects.is_empty() {
        return Err(AppError(anyhow!(
            "select at least one of: face, body, posture, clothing, environment"
        )));
    }

    let row = sqlx::query_as::<_, (String,)>("SELECT path FROM media WHERE id = ?")
        .bind(&req.media_id)
        .fetch_optional(&st.pool)
        .await?;
    let Some((path,)) = row else {
        return Err(AppError(anyhow!("image not found: {}", req.media_id)));
    };

    if st.cfg.mock {
        return Ok(Json(DescribeResp {
            prompt: mock_prompt(&aspects),
            described: false,
        }));
    }

    let bytes = tokio::fs::read(&path).await?;
    // Vision models (e.g. Gemma) reject formats like WebP. Normalise to PNG so any
    // uploaded image works; fall back to the raw bytes if it can't be decoded.
    let (img_bytes, mime): (Vec<u8>, &str) = match normalize_to_png(&bytes) {
        Some(png) => (png, "image/png"),
        None => (bytes, mime_from_path(&path)),
    };
    let prompt =
        crate::clients::lmstudio::describe_image(&st, &img_bytes, mime, &aspects).await?;
    Ok(Json(DescribeResp {
        prompt,
        described: true,
    }))
}

/// Decode any supported image (png/jpeg/webp/gif) and re-encode as PNG.
fn normalize_to_png(bytes: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some(buf.into_inner())
}

fn mock_prompt(aspects: &[String]) -> String {
    format!(
        "score_9, score_8_up, source_photo, raw photo, photorealistic, [mock description covering: {}], \
detailed skin texture, natural lighting, 8k",
        aspects.join(", ")
    )
}

fn mime_from_path(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png"
    }
}

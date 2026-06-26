use anyhow::anyhow;
use axum::{
    extract::{Multipart, Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::AppState;

/// `POST /api/media` — multipart upload of a source/target image. Returns `{ id }`.
pub async fn upload(
    State(st): State<AppState>,
    mut mp: Multipart,
) -> AppResult<Json<serde_json::Value>> {
    while let Some(field) = mp.next_field().await? {
        let filename = field.file_name().map(|s| s.to_string());
        let content_type = field.content_type().map(|s| s.to_string());
        let data = field.bytes().await?;

        let ext = ext_for(filename.as_deref(), content_type.as_deref());
        let id = Uuid::new_v4().to_string();
        tokio::fs::create_dir_all(&st.cfg.media_dir).await?;
        let path = format!("{}/{}.{}", st.cfg.media_dir, id, ext);
        tokio::fs::write(&path, &data).await?;

        sqlx::query("INSERT INTO media (id, path, kind) VALUES (?, ?, 'upload')")
            .bind(&id)
            .bind(&path)
            .execute(&st.pool)
            .await?;

        return Ok(Json(json!({ "id": id })));
    }
    Err(anyhow!("no file field in multipart body").into())
}

/// `GET /api/media/:id` — serve a stored media file over Tailscale.
pub async fn serve(State(st): State<AppState>, Path(id): Path<String>) -> AppResult<Response> {
    let row = sqlx::query_as::<_, (String,)>("SELECT path FROM media WHERE id = ?")
        .bind(&id)
        .fetch_optional(&st.pool)
        .await?;

    let Some((path,)) = row else {
        return Ok((StatusCode::NOT_FOUND, "media not found").into_response());
    };

    let bytes = match tokio::fs::read(&path).await {
        Ok(b) => b,
        Err(_) => return Ok((StatusCode::NOT_FOUND, "media file missing").into_response()),
    };

    let ct = mime_from_path(&path);
    Ok(([(header::CONTENT_TYPE, ct)], bytes).into_response())
}

fn ext_for(filename: Option<&str>, content_type: Option<&str>) -> String {
    if let Some(name) = filename {
        if let Some(dot) = name.rfind('.') {
            let e = name[dot + 1..].to_lowercase();
            if !e.is_empty() && e.len() <= 5 {
                return e;
            }
        }
    }
    match content_type {
        Some("image/png") => "png",
        Some("image/jpeg") => "jpg",
        Some("image/webp") => "webp",
        Some("video/mp4") => "mp4",
        Some("image/gif") => "gif",
        _ => "bin",
    }
    .to_string()
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
    } else if lower.ends_with(".mp4") {
        "video/mp4"
    } else {
        "application/octet-stream"
    }
}

/// `DELETE /api/media/:id` — delete a media file from disk and DB.
/// Also strips references to this media from any jobs.
pub async fn delete(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    let row = sqlx::query_as::<_, (String,)>("SELECT path FROM media WHERE id = ?")
        .bind(&id)
        .fetch_optional(&st.pool)
        .await?;

    let Some((path,)) = row else {
        return Err(AppError(anyhow!("media not found")));
    };

    // Remove the file from disk (ignore missing-file errors).
    let _ = tokio::fs::remove_file(&path).await;

    // Strip this media id from job references.
    // SQLite doesn't have a built-in JSON array remove, so we fetch,
    // filter in Rust, and update each affected job.
    let jobs: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, input_media_ids, output_media_ids FROM jobs
         WHERE input_media_ids LIKE ? OR output_media_ids LIKE ?",
    )
    .bind(format!("%{id}%"))
    .bind(format!("%{id}%"))
    .fetch_all(&st.pool)
    .await?;

    for (job_id, inputs_json, outputs_json) in &jobs {
        let mut inputs: Vec<String> =
            serde_json::from_str(inputs_json).unwrap_or_default();
        let mut outputs: Vec<String> =
            serde_json::from_str(outputs_json).unwrap_or_default();
        inputs.retain(|mid| mid != &id);
        outputs.retain(|mid| mid != &id);
        sqlx::query(
            "UPDATE jobs SET input_media_ids = ?, output_media_ids = ?,
             updated_at = datetime('now') WHERE id = ?",
        )
        .bind(serde_json::to_string(&inputs).unwrap_or_default())
        .bind(serde_json::to_string(&outputs).unwrap_or_default())
        .bind(job_id)
        .execute(&st.pool)
        .await?;
    }

    // Delete the media row itself.
    sqlx::query("DELETE FROM media WHERE id = ?")
        .bind(&id)
        .execute(&st.pool)
        .await?;

    Ok(Json(json!({"deleted": id})))
}

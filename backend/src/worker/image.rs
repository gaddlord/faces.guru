use anyhow::Result;
use serde_json::Value;

use super::{placeholder_png, save_media, set_progress};
use crate::models::Job;
use crate::AppState;

/// Image generation (§3.1 / §3.2). Reads positive/negative/reference conditioning from
/// `params_json`, runs SDXL/Flux via ComfyUI (or fabricates a placeholder in mock mode).
pub async fn run(st: &AppState, job: &Job, params: &Value) -> Result<Vec<String>> {
    let positive = params["positive"]
        .as_str()
        .or_else(|| params["prompt"].as_str())
        .unwrap_or("")
        .to_string();
    let negative = params["negative"].as_str().unwrap_or("").to_string();
    let width = params["width"].as_u64().unwrap_or(1024) as u32;
    let height = params["height"].as_u64().unwrap_or(1024) as u32;
    let steps = params["steps"].as_u64().unwrap_or(30) as u32;
    let cfg_scale = params["cfg"].as_f64().unwrap_or(6.0) as f32;
    let seed = params["seed"].as_i64().unwrap_or_else(|| seed_from(&job.id));
    // Photoreal-friendly defaults (Pony/Illustrious realism look better on dpmpp_2m + karras
    // than the SDXL default euler/normal). Overridable per job.
    let sampler = params["sampler"].as_str().unwrap_or("dpmpp_2m").to_string();
    let scheduler = params["scheduler"].as_str().unwrap_or("karras").to_string();
    // Optional VAE override (set if a checkpoint produces washed-out colors).
    let vae = params["vae"].as_str().filter(|s| !s.trim().is_empty());

    set_progress(st, &job.id, 0.1).await?;

    if st.cfg.mock {
        // Simulate a few progress ticks, then emit a deterministic gradient image.
        for p in [0.35_f64, 0.7] {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            set_progress(st, &job.id, p).await?;
        }
        let seed_text = format!("{positive}|{negative}|{seed}");
        let png = placeholder_png(&seed_text, width.min(1024), height.min(1024))?;
        let id = save_media(st, "image", "png", &png).await?;
        return Ok(vec![id]);
    }

    set_progress(st, &job.id, 0.3).await?;
    let png = crate::clients::comfyui::text2img(
        st, &positive, &negative, width, height, steps, cfg_scale, seed, &sampler, &scheduler, vae,
    )
    .await?;
    let id = save_media(st, "image", "png", &png).await?;
    Ok(vec![id])
}

fn seed_from(s: &str) -> i64 {
    let mut hash: i64 = 0;
    for b in s.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(b as i64);
    }
    hash.abs()
}

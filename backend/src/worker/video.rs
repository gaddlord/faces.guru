use anyhow::{anyhow, Result};
use serde_json::Value;

use super::{placeholder_gif, read_media, save_media, set_progress};
use crate::models::Job;
use crate::AppState;

/// Image → video (§3.4). `input_media_ids` = [still_image]; params: motion prompt,
/// fps, duration. Slow on Apple Silicon — output is capped. Mock emits a GIF placeholder.
pub async fn run(st: &AppState, job: &Job, params: &Value, inputs: &[String]) -> Result<Vec<String>> {
    let still_id = inputs
        .first()
        .ok_or_else(|| anyhow!("video requires one input_media_id: [still_image]"))?;

    let prompt = params["prompt"].as_str().unwrap_or("").to_string();
    // Conservative v1 caps (§3.4): short clips, modest fps.
    let fps = (params["fps"].as_u64().unwrap_or(24) as u32).clamp(8, 30);
    let duration = (params["duration"].as_f64().unwrap_or(4.0) as f32).clamp(1.0, 6.0);

    set_progress(st, &job.id, 0.1).await?;
    let image_bytes = read_media(st, still_id).await?;

    if st.cfg.mock {
        // Simulate frame-by-frame progress.
        let frames = (fps as f32 * duration) as u32;
        for i in 1..=4u32 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            set_progress(st, &job.id, 0.1 + 0.8 * (i as f64 / 4.0)).await?;
        }
        let seed_text = format!("{prompt}|{frames}");
        let gif = placeholder_gif(&seed_text, 512, 512)?;
        let id = save_media(st, "video", "gif", &gif).await?;
        return Ok(vec![id]);
    }

    set_progress(st, &job.id, 0.3).await?;
    // Normalize to PNG so the video service isn't tripped up by WebP/other formats.
    let image_bytes = crate::imageutil::to_png_or_original(image_bytes);
    let mp4 = crate::clients::videosvc::img2vid(st, image_bytes, &prompt, fps, duration).await?;
    let id = save_media(st, "video", "mp4", &mp4).await?;
    Ok(vec![id])
}

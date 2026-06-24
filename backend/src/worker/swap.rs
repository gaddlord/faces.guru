use anyhow::{anyhow, Result};
use serde_json::Value;

use super::{read_media, save_media, set_progress};
use crate::models::Job;
use crate::AppState;

/// Face swap (§3.3). `input_media_ids` = [source_face, target_image]; `params.restore`
/// toggles GFPGAN/CodeFormer restoration. In mock mode, echoes the target back.
pub async fn run(st: &AppState, job: &Job, params: &Value, inputs: &[String]) -> Result<Vec<String>> {
    if inputs.len() < 2 {
        return Err(anyhow!(
            "swap requires two input_media_ids: [source_face, target_image]"
        ));
    }
    let restore = params["restore"].as_bool().unwrap_or(true);

    set_progress(st, &job.id, 0.15).await?;
    let source = read_media(st, &inputs[0]).await?;
    let target = read_media(st, &inputs[1]).await?;

    if st.cfg.mock {
        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        set_progress(st, &job.id, 0.6).await?;
        // Mock: re-emit the target as a placeholder so the flow completes end-to-end.
        let _ = source;
        let ext = if target.len() > 8 && &target[0..8] == b"\x89PNG\r\n\x1a\n" {
            "png"
        } else {
            "jpg"
        };
        let id = save_media(st, "swap", ext, &target).await?;
        return Ok(vec![id]);
    }

    set_progress(st, &job.id, 0.4).await?;
    let result = crate::clients::faceswap::swap(st, source, target, restore).await?;
    let id = save_media(st, "swap", "png", &result).await?;
    Ok(vec![id])
}

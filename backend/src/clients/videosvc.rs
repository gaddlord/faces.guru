use anyhow::{anyhow, Result};
use reqwest::multipart;
use std::time::Duration;

use crate::AppState;

/// Call the local img2vid microservice (LTX-Video / Wan 2.2). Contract:
///
///   POST {FG_VIDEO_URL}/generate   (multipart/form-data)
///     - field "image":    the still to animate
///     - field "prompt":   motion prompt
///     - field "fps":      target fps (e.g. "24")
///     - field "duration": seconds (e.g. "4")
///   => 200 with an mp4 (video/mp4)
///
/// Video is slow on Apple Silicon, so the timeout is generous (20 min).
pub async fn img2vid(
    st: &AppState,
    image_bytes: Vec<u8>,
    prompt: &str,
    fps: u32,
    duration: f32,
) -> Result<Vec<u8>> {
    let url = format!("{}/generate", st.cfg.video_url.trim_end_matches('/'));

    let form = multipart::Form::new()
        .part(
            "image",
            multipart::Part::bytes(image_bytes).file_name("input.png"),
        )
        .text("prompt", prompt.to_string())
        .text("fps", fps.to_string())
        .text("duration", duration.to_string());

    let resp = st
        .http
        .post(&url)
        .multipart(form)
        .timeout(Duration::from_secs(1200))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("video service returned status {}", resp.status()));
    }

    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

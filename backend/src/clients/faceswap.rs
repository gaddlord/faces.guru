use anyhow::{anyhow, Result};
use reqwest::multipart;
use std::time::Duration;

use crate::AppState;

/// Call the local face-swap microservice (InsightFace detect + InSwapper, then
/// GFPGAN/CodeFormer restore). Contract:
///
///   POST {FG_FACESWAP_URL}/swap   (multipart/form-data)
///     - field "source": the face to take
///     - field "target": the image to paste the face onto
///     - field "restore": "true" | "false" (optional)
///   => 200 with the resulting image bytes (image/png or image/jpeg)
///
/// `source_bytes` / `target_bytes` are the raw image files.
pub async fn swap(
    st: &AppState,
    source_bytes: Vec<u8>,
    target_bytes: Vec<u8>,
    restore: bool,
) -> Result<Vec<u8>> {
    let url = format!("{}/swap", st.cfg.faceswap_url.trim_end_matches('/'));

    let form = multipart::Form::new()
        .part(
            "source",
            multipart::Part::bytes(source_bytes).file_name("source.png"),
        )
        .part(
            "target",
            multipart::Part::bytes(target_bytes).file_name("target.png"),
        )
        .text("restore", if restore { "true" } else { "false" });

    let resp = st
        .http
        .post(&url)
        .multipart(form)
        .timeout(Duration::from_secs(180))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("face-swap service returned status {}", resp.status()));
    }

    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use std::time::Duration;
use uuid::Uuid;

use crate::AppState;

/// Generate an image from a text prompt via a local ComfyUI server (SDXL graph).
/// Returns the raw PNG bytes of the first output image.
#[allow(clippy::too_many_arguments)]
pub async fn text2img(
    st: &AppState,
    positive: &str,
    negative: &str,
    width: u32,
    height: u32,
    steps: u32,
    cfg_scale: f32,
    seed: i64,
    sampler: &str,
    scheduler: &str,
    vae: Option<&str>,
) -> Result<Vec<u8>> {
    let workflow = build_sdxl_workflow(
        &st.cfg.image_ckpt, positive, negative, width, height, steps, cfg_scale, seed, sampler,
        scheduler, vae,
    );
    submit_and_collect(st, workflow).await
}

/// Submit a prebuilt workflow graph and download the first resulting image.
/// Shared by image and (future) ComfyUI-based video paths.
pub async fn submit_and_collect(st: &AppState, workflow: Value) -> Result<Vec<u8>> {
    let base = st.cfg.comfyui_url.trim_end_matches('/');
    let client_id = Uuid::new_v4().to_string();

    let submit: Value = st
        .http
        .post(format!("{base}/prompt"))
        .json(&json!({ "prompt": workflow, "client_id": client_id }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let prompt_id = submit["prompt_id"]
        .as_str()
        .ok_or_else(|| anyhow!("ComfyUI did not return a prompt_id"))?
        .to_string();

    // Poll history until the prompt finishes (cap ~10 minutes).
    for _ in 0..600 {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let hist: Value = st
            .http
            .get(format!("{base}/history/{prompt_id}"))
            .send()
            .await?
            .json()
            .await?;

        let Some(entry) = hist.get(&prompt_id) else {
            continue;
        };
        let Some(outputs) = entry.get("outputs").and_then(|o| o.as_object()) else {
            continue;
        };

        for (_node_id, out) in outputs {
            if let Some(images) = out.get("images").and_then(|i| i.as_array()) {
                if let Some(img) = images.first() {
                    let filename = img["filename"].as_str().unwrap_or_default();
                    let subfolder = img["subfolder"].as_str().unwrap_or_default();
                    let typ = img["type"].as_str().unwrap_or("output");
                    let bytes = st
                        .http
                        .get(format!("{base}/view"))
                        .query(&[("filename", filename), ("subfolder", subfolder), ("type", typ)])
                        .send()
                        .await?
                        .error_for_status()?
                        .bytes()
                        .await?;
                    return Ok(bytes.to_vec());
                }
            }
        }
    }

    Err(anyhow!("ComfyUI job timed out before producing an image"))
}

/// The canonical default ComfyUI SDXL text-to-image graph.
/// The canonical default ComfyUI SDXL text-to-image graph. When `vae` is Some, an extra
/// VAELoader node is added and VAEDecode draws from it instead of the checkpoint's baked VAE.
#[allow(clippy::too_many_arguments)]
fn build_sdxl_workflow(
    ckpt: &str,
    positive: &str,
    negative: &str,
    width: u32,
    height: u32,
    steps: u32,
    cfg_scale: f32,
    seed: i64,
    sampler: &str,
    scheduler: &str,
    vae: Option<&str>,
) -> Value {
    let vae_ref = if vae.is_some() {
        json!(["10", 0])
    } else {
        json!(["4", 2])
    };

    let mut graph = json!({
        "4": {
            "class_type": "CheckpointLoaderSimple",
            "inputs": { "ckpt_name": ckpt }
        },
        "5": {
            "class_type": "EmptyLatentImage",
            "inputs": { "width": width, "height": height, "batch_size": 1 }
        },
        "6": {
            "class_type": "CLIPTextEncode",
            "inputs": { "text": positive, "clip": ["4", 1] }
        },
        "7": {
            "class_type": "CLIPTextEncode",
            "inputs": { "text": negative, "clip": ["4", 1] }
        },
        "3": {
            "class_type": "KSampler",
            "inputs": {
                "seed": seed,
                "steps": steps,
                "cfg": cfg_scale,
                "sampler_name": sampler,
                "scheduler": scheduler,
                "denoise": 1.0,
                "model": ["4", 0],
                "positive": ["6", 0],
                "negative": ["7", 0],
                "latent_image": ["5", 0]
            }
        },
        "8": {
            "class_type": "VAEDecode",
            "inputs": { "samples": ["3", 0], "vae": vae_ref }
        },
        "9": {
            "class_type": "SaveImage",
            "inputs": { "images": ["8", 0], "filename_prefix": "facesguru" }
        }
    });

    if let Some(vae_name) = vae {
        graph["10"] = json!({
            "class_type": "VAELoader",
            "inputs": { "vae_name": vae_name }
        });
    }

    graph
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_without_vae_uses_checkpoint_vae() {
        let g = build_sdxl_workflow(
            "ckpt.safetensors", "p", "n", 1024, 1024, 30, 6.0, 42, "dpmpp_2m", "karras", None,
        );
        assert_eq!(g["8"]["inputs"]["vae"], json!(["4", 2]));
        assert!(g.get("10").is_none());
        assert_eq!(g["3"]["inputs"]["sampler_name"], "dpmpp_2m");
        assert_eq!(g["3"]["inputs"]["scheduler"], "karras");
    }

    #[test]
    fn workflow_with_vae_adds_loader_node() {
        let g = build_sdxl_workflow(
            "ckpt.safetensors", "p", "n", 832, 1216, 30, 6.0, 42, "euler", "normal",
            Some("sdxl_vae.safetensors"),
        );
        assert_eq!(g["8"]["inputs"]["vae"], json!(["10", 0]));
        assert_eq!(g["10"]["class_type"], "VAELoader");
        assert_eq!(g["10"]["inputs"]["vae_name"], "sdxl_vae.safetensors");
    }
}

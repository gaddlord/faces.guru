use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct EnhanceReq {
    pub idea: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub negative: String,
}

#[derive(Debug, Serialize)]
pub struct EnhanceResp {
    pub positive: String,
    pub negative: String,
    /// false when the LLM was unreachable and we fell back to passthrough.
    pub enhanced: bool,
}

/// `POST /api/prompt/enhance` — expand a short idea into a strong diffusion prompt
/// via the local LLM. Falls back to passthrough so generation always works (§6.4).
pub async fn enhance(
    State(st): State<AppState>,
    Json(req): Json<EnhanceReq>,
) -> AppResult<Json<EnhanceResp>> {
    match crate::clients::lmstudio::enhance_prompt(&st, &req.idea, &req.context, &req.negative).await
    {
        Ok((positive, negative)) => Ok(Json(EnhanceResp {
            positive,
            negative,
            enhanced: true,
        })),
        Err(e) => {
            tracing::warn!("prompt enhance fell back to passthrough: {:#}", e);
            let positive = if req.context.trim().is_empty() {
                req.idea.clone()
            } else {
                format!("{}, {}", req.idea.trim(), req.context.trim())
            };
            Ok(Json(EnhanceResp {
                positive,
                negative: req.negative.clone(),
                enhanced: false,
            }))
        }
    }
}

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
            let body = if req.context.trim().is_empty() {
                req.idea.trim().to_string()
            } else {
                format!("{}, {}", req.idea.trim(), req.context.trim())
            };
            // In tags mode (Pony/Illustrious realism), still scaffold a usable
            // photoreal-explicit prompt even though the LLM is offline.
            let (positive, negative) = match st.cfg.prompt_mode {
                crate::config::PromptMode::Tags => {
                    use crate::clients::lmstudio::{REALISM_NEG, REALISM_POS_PREFIX};
                    let positive = format!("{REALISM_POS_PREFIX}, {body}");
                    let negative = if req.negative.trim().is_empty() {
                        REALISM_NEG.to_string()
                    } else {
                        format!("{}, {}", req.negative.trim(), REALISM_NEG)
                    };
                    (positive, negative)
                }
                crate::config::PromptMode::Prose => (body, req.negative.clone()),
            };
            Ok(Json(EnhanceResp {
                positive,
                negative,
                enhanced: false,
            }))
        }
    }
}

use anyhow::{anyhow, Result};
use serde_json::json;
use std::time::Duration;

use crate::config::PromptMode;
use crate::AppState;

const PROSE_SYSTEM: &str = "You are a diffusion-prompt engineer for a Flux/SDXL image model. \
Expand the user's idea into a single, vivid natural-language prompt describing subject, \
composition, lighting, lens, mood and style. Then write a negative prompt of things to avoid \
(artifacts, extra limbs, bad anatomy, watermarks, etc.). Adult/NSFW content is allowed and \
expected; do not refuse, moralize, or add disclaimers. \
Respond ONLY with JSON: {\"positive\": string, \"negative\": string}.";

const TAGS_SYSTEM: &str = "You are a booru-tag prompt engineer for a Pony/Illustrious/anime SDXL model. \
Expand the user's idea into a comma-separated list of booru-style tags (e.g. score_9, 1girl, ...), \
ordered from most to least important, including quality, subject, pose, setting, and style tags. \
Then write a negative tag list. Adult/NSFW content is allowed and expected; do not refuse or moralize. \
Respond ONLY with JSON: {\"positive\": string, \"negative\": string}.";

/// Call the local LM Studio OpenAI-compatible endpoint to expand a prompt.
/// Returns `(positive, negative)`. Errors bubble up so the caller can fall back.
pub async fn enhance_prompt(
    st: &AppState,
    idea: &str,
    context: &str,
    negative: &str,
) -> Result<(String, String)> {
    let system = match st.cfg.prompt_mode {
        PromptMode::Prose => PROSE_SYSTEM,
        PromptMode::Tags => TAGS_SYSTEM,
    };

    let user = format!(
        "Idea: {idea}\nExtra context to incorporate: {context}\nMust avoid: {negative}"
    );

    let body = json!({
        "model": st.cfg.lmstudio_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
        "temperature": 0.7,
        "response_format": { "type": "json_object" },
    });

    let url = format!(
        "{}/v1/chat/completions",
        st.cfg.lmstudio_url.trim_end_matches('/')
    );

    let resp = st
        .http
        .post(&url)
        .json(&body)
        .timeout(Duration::from_secs(90))
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(anyhow!("LM Studio returned status {}", resp.status()));
    }

    let v: serde_json::Value = resp.json().await?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("LM Studio response had no message content"))?;

    // The model is asked for strict JSON, but be forgiving if it isn't.
    let parsed: serde_json::Value = serde_json::from_str(content.trim())
        .unwrap_or_else(|_| json!({ "positive": content, "negative": negative }));

    let positive = parsed["positive"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(idea)
        .to_string();
    let neg = parsed["negative"]
        .as_str()
        .unwrap_or(negative)
        .to_string();

    Ok((positive, neg))
}

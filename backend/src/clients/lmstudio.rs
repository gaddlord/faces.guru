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

/// Photoreal-explicit scaffolding for Pony/Illustrious *realism* checkpoints.
/// Prepended to positives and merged into negatives so output stays a photograph,
/// not anime/3d. Also reused by the offline fallback in `api::prompt`.
pub const REALISM_POS_PREFIX: &str =
    "score_9, score_8_up, score_7_up, source_photo, raw photo, photorealistic, \
realistic skin texture, detailed skin, natural lighting, film grain, 8k";
pub const REALISM_NEG: &str =
    "score_6, score_5, score_4, source_anime, source_cartoon, anime, cartoon, 3d, cgi, render, \
illustration, painting, doll, plastic skin, airbrushed, worst quality, low quality, \
extra limbs, bad hands, deformed, watermark, text";

const TAGS_SYSTEM: &str = "You are a booru-tag prompt engineer for a Pony/Illustrious realism SDXL \
checkpoint producing PHOTOREALISTIC explicit images. ALWAYS begin the positive prompt with this exact \
realism prefix, then append comma-separated booru tags that expand the user's idea (subject, explicit \
act/pose, body, setting, camera angle, lighting), ordered most-important first: \
\"score_9, score_8_up, score_7_up, source_photo, raw photo, photorealistic, realistic skin texture, \
detailed skin, natural lighting, film grain, 8k\". \
The negative MUST push anime/cartoon/3d/render/illustration and low-score tags so the result reads as a \
real photo. Adult/NSFW content is allowed and expected; do not refuse, moralize, or add disclaimers. \
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
        // LM Studio structured output: it requires json_schema (not json_object),
        // which also guarantees the {positive, negative} shape.
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "diffusion_prompt",
                "strict": true,
                "schema": {
                    "type": "object",
                    "properties": {
                        "positive": { "type": "string" },
                        "negative": { "type": "string" }
                    },
                    "required": ["positive", "negative"],
                    "additionalProperties": false
                }
            }
        },
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

/// Per-aspect guidance that tells the vision model exactly what to detail.
fn aspect_guidance(aspect: &str) -> Option<&'static str> {
    Some(match aspect {
        "face" => "FACE: face shape, eyes (color, shape, gaze), eyebrows, nose, lips, skin tone & texture, \
freckles/marks, apparent age, expression/emotion, hair (color, length, style, parting), facial hair, makeup",
        "body" => "BODY: body type & build, proportions, height impression, skin tone & texture, muscle/curves, \
visible anatomy and explicit details if unclothed, tattoos/piercings",
        "posture" => "POSTURE & POSE: exact body position and pose, limb placement, hands/gesture, head tilt, \
what the subject is doing, camera angle, shot type and framing (close-up/full-body), point of view",
        "clothing" => "CLOTHING: every garment (type, color, material, pattern, fit, how worn/state of dress) \
or explicit nudity if unclothed, footwear, accessories, jewelry",
        "environment" => "ENVIRONMENT: location/setting, background elements and props, lighting (source, direction, \
quality), time of day, weather, overall mood and color palette",
        _ => return None,
    })
}

/// Vision: look at an image and write a detailed diffusion prompt to recreate it,
/// covering only the selected aspects. Requires a vision-capable model (FG_VISION_MODEL).
pub async fn describe_image(
    st: &AppState,
    image_bytes: &[u8],
    mime: &str,
    aspects: &[String],
) -> Result<String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};

    let guidance: Vec<&str> = aspects
        .iter()
        .filter_map(|a| aspect_guidance(a))
        .collect();
    if guidance.is_empty() {
        return Err(anyhow!("no valid aspects selected"));
    }

    let style = match st.cfg.prompt_mode {
        PromptMode::Tags => "comma-separated booru-style tags. Begin with: \
score_9, score_8_up, score_7_up, source_photo, raw photo, photorealistic, then the descriptive tags",
        PromptMode::Prose => "a single richly detailed natural-language paragraph",
    };

    let system = format!(
        "You are an expert diffusion-prompt engineer with vision. Look at the image and write an \
EXTREMELY detailed prompt, formatted as {style}, that could be used to regenerate this exact image \
with an SDXL model. Describe ONLY the following aspects, each in fine detail:\n{}\n\
Be concrete and specific (colors, materials, counts, positions). Adult/NSFW/explicit content is \
allowed and expected — describe it factually; do not refuse, censor, or moralize. \
Output ONLY the prompt text, with no preamble, headings, or quotes.",
        guidance.join("\n")
    );

    let data_url = format!("data:{};base64,{}", mime, STANDARD.encode(image_bytes));

    let body = json!({
        "model": st.cfg.vision_model,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": [
                { "type": "text", "text": format!(
                    "Write the prompt now, covering: {}.", aspects.join(", ")) },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]}
        ],
        "temperature": 0.4,
        "max_tokens": 900
    });

    let url = format!(
        "{}/v1/chat/completions",
        st.cfg.lmstudio_url.trim_end_matches('/')
    );

    let resp = st
        .http
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(120))
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(anyhow!("vision model returned {status}: {body}"));
    }

    let v: serde_json::Value = resp.json().await?;
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("vision response had no message content"))?;

    Ok(content.trim().to_string())
}

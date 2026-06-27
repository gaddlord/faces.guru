use std::env;

/// Which prompt-assist style the enhancer LLM should emit.
/// Flux wants natural-language prose; Pony/Illustrious/anime SDXL want booru tags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptMode {
    Prose,
    Tags,
}

/// Runtime configuration, loaded from environment (see `.env.example`).
#[derive(Clone, Debug)]
pub struct Config {
    pub bind: String,
    pub db_url: String,
    pub media_dir: String,
    /// When true, the worker fabricates placeholder media instead of calling real models,
    /// so the whole app runs end-to-end without any model servers.
    pub mock: bool,
    pub prompt_mode: PromptMode,
    pub comfyui_url: String,
    pub image_ckpt: String,
    pub lmstudio_url: String,
    pub lmstudio_model: String,
    /// Vision-capable model id for image description (must support image input).
    /// Defaults to `lmstudio_model`; override with FG_VISION_MODEL.
    pub vision_model: String,
    pub faceswap_url: String,
    pub video_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        let get = |k: &str, d: &str| env::var(k).unwrap_or_else(|_| d.to_string());
        let mock = get("FG_MOCK", "true").eq_ignore_ascii_case("true");
        let prompt_mode = match get("FG_PROMPT_MODE", "prose").to_lowercase().as_str() {
            "tags" => PromptMode::Tags,
            _ => PromptMode::Prose,
        };
        Config {
            bind: get("FG_BIND", "0.0.0.0:8080"),
            db_url: get("FG_DB", "sqlite://data/facesguru.db"),
            media_dir: get("FG_MEDIA_DIR", "data/media"),
            mock,
            prompt_mode,
            comfyui_url: get("FG_COMFYUI_URL", "http://127.0.0.1:8188"),
            image_ckpt: get("FG_IMAGE_CKPT", "sd_xl_base_1.0.safetensors"),
            lmstudio_url: get("FG_LMSTUDIO_URL", "http://127.0.0.1:1234"),
            lmstudio_model: get("FG_LMSTUDIO_MODEL", "mistral-small-24b-instruct"),
            vision_model: get(
                "FG_VISION_MODEL",
                &get("FG_LMSTUDIO_MODEL", "mistral-small-24b-instruct"),
            ),
            faceswap_url: get("FG_FACESWAP_URL", "http://127.0.0.1:5000"),
            video_url: get("FG_VIDEO_URL", "http://127.0.0.1:5001"),
        }
    }
}

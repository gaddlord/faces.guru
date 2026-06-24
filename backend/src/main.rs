mod api;
mod clients;
mod config;
mod db;
mod error;
mod models;
mod worker;

use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use sqlx::sqlite::SqlitePool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::config::Config;

/// Shared state handed to every handler and to the worker.
#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub cfg: Arc<Config>,
    pub http: reqwest::Client,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,sqlx=warn".into()),
        )
        .init();

    let cfg = Arc::new(Config::from_env());
    tracing::info!(
        "starting faces.guru backend (mock={}, prompt_mode={:?})",
        cfg.mock,
        cfg.prompt_mode
    );

    let pool = db::init_pool(&cfg.db_url).await?;
    let http = reqwest::Client::builder()
        .build()
        .expect("failed to build http client");

    let state = AppState {
        pool,
        cfg: cfg.clone(),
        http,
    };

    // Background worker: drains the local jobs queue serially.
    tokio::spawn(worker::run(state.clone()));

    let app = Router::new()
        .route("/health", get(health))
        .route("/api/prompt/enhance", post(api::prompt::enhance))
        .route("/api/jobs", post(api::jobs::create).get(api::jobs::list))
        .route("/api/jobs/:id", get(api::jobs::get_one))
        .route("/api/media", post(api::media::upload))
        .route("/api/media/:id", get(api::media::serve))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(&cfg.bind).await?;
    tracing::info!("listening on http://{}", cfg.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({ "status": "ok", "service": "facesguru-backend" }))
}

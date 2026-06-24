use anyhow::anyhow;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use sqlx::sqlite::SqlitePool;
use uuid::Uuid;

use crate::error::AppResult;
use crate::models::{CreateJob, Job, JobResponse};
use crate::AppState;

/// `POST /api/jobs` — enqueue an image / swap / video job. Returns the created job.
pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreateJob>,
) -> AppResult<Json<JobResponse>> {
    match req.job_type.as_str() {
        "image" | "swap" | "video" => {}
        other => return Err(anyhow!("invalid job type '{other}' (expected image|swap|video)").into()),
    }

    let id = Uuid::new_v4().to_string();
    let params = serde_json::to_string(&req.params)?;
    let inputs = serde_json::to_string(&req.input_media_ids)?;

    sqlx::query(
        "INSERT INTO jobs (id, job_type, status, params_json, input_media_ids)
         VALUES (?, ?, 'queued', ?, ?)",
    )
    .bind(&id)
    .bind(&req.job_type)
    .bind(&params)
    .bind(&inputs)
    .execute(&st.pool)
    .await?;

    let job = fetch_job(&st.pool, &id).await?;
    Ok(Json(job.into()))
}

/// `GET /api/jobs/:id` — poll a single job's status/progress/outputs.
pub async fn get_one(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<axum::response::Response> {
    let job = sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = ?")
        .bind(&id)
        .fetch_optional(&st.pool)
        .await?;

    match job {
        Some(j) => Ok(Json(JobResponse::from(j)).into_response()),
        None => Ok((StatusCode::NOT_FOUND, "job not found").into_response()),
    }
}

/// `GET /api/jobs` — recent history/gallery, newest first.
pub async fn list(State(st): State<AppState>) -> AppResult<Json<Vec<JobResponse>>> {
    let jobs = sqlx::query_as::<_, Job>("SELECT * FROM jobs ORDER BY created_at DESC LIMIT 200")
        .fetch_all(&st.pool)
        .await?;
    Ok(Json(jobs.into_iter().map(JobResponse::from).collect()))
}

pub async fn fetch_job(pool: &SqlitePool, id: &str) -> anyhow::Result<Job> {
    let job = sqlx::query_as::<_, Job>("SELECT * FROM jobs WHERE id = ?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(job)
}

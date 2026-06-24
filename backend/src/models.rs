use serde::{Deserialize, Serialize};
use sqlx::FromRow;

/// A row from the `jobs` table.
#[derive(Debug, Clone, FromRow)]
pub struct Job {
    pub id: String,
    pub job_type: String,
    pub status: String,
    pub params_json: String,
    pub input_media_ids: String,
    pub output_media_ids: String,
    pub error: Option<String>,
    pub progress: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl Job {
    pub fn inputs(&self) -> Vec<String> {
        serde_json::from_str(&self.input_media_ids).unwrap_or_default()
    }
    pub fn params(&self) -> serde_json::Value {
        serde_json::from_str(&self.params_json).unwrap_or(serde_json::Value::Null)
    }
}

/// Public-facing job shape (params/media arrays expanded from their stored JSON strings).
#[derive(Debug, Serialize)]
pub struct JobResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub job_type: String,
    pub status: String,
    pub params: serde_json::Value,
    pub input_media_ids: Vec<String>,
    pub output_media_ids: Vec<String>,
    pub error: Option<String>,
    pub progress: f64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Job> for JobResponse {
    fn from(j: Job) -> Self {
        JobResponse {
            params: serde_json::from_str(&j.params_json).unwrap_or(serde_json::Value::Null),
            input_media_ids: serde_json::from_str(&j.input_media_ids).unwrap_or_default(),
            output_media_ids: serde_json::from_str(&j.output_media_ids).unwrap_or_default(),
            id: j.id,
            job_type: j.job_type,
            status: j.status,
            error: j.error,
            progress: j.progress,
            created_at: j.created_at,
            updated_at: j.updated_at,
        }
    }
}

/// Request body for `POST /api/jobs`.
#[derive(Debug, Deserialize)]
pub struct CreateJob {
    #[serde(rename = "type")]
    pub job_type: String,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub input_media_ids: Vec<String>,
}

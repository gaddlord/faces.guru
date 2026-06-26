use anyhow::anyhow;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::AppState;

#[derive(Debug, FromRow)]
struct PresetRow {
    id: String,
    name: String,
    kind: String,
    data_json: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct Preset {
    id: String,
    name: String,
    kind: String,
    data: serde_json::Value,
    created_at: String,
    updated_at: String,
}

impl From<PresetRow> for Preset {
    fn from(r: PresetRow) -> Self {
        Preset {
            data: serde_json::from_str(&r.data_json).unwrap_or(serde_json::Value::Null),
            id: r.id,
            name: r.name,
            kind: r.kind,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreatePreset {
    pub name: String,
    #[serde(default = "default_kind")]
    pub kind: String,
    #[serde(default)]
    pub data: serde_json::Value,
}

fn default_kind() -> String {
    "image".to_string()
}

#[derive(Debug, Deserialize)]
pub struct UpdatePreset {
    pub name: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// `GET /api/presets?kind=image` — list saved presets (newest naming sorts by name).
pub async fn list(
    State(st): State<AppState>,
    Query(q): Query<HashMap<String, String>>,
) -> AppResult<Json<Vec<Preset>>> {
    let rows = if let Some(kind) = q.get("kind") {
        sqlx::query_as::<_, PresetRow>(
            "SELECT * FROM presets WHERE kind = ? ORDER BY name COLLATE NOCASE",
        )
        .bind(kind)
        .fetch_all(&st.pool)
        .await?
    } else {
        sqlx::query_as::<_, PresetRow>("SELECT * FROM presets ORDER BY name COLLATE NOCASE")
            .fetch_all(&st.pool)
            .await?
    };
    Ok(Json(rows.into_iter().map(Preset::from).collect()))
}

/// `POST /api/presets` — save current settings under a new name ("Save As").
pub async fn create(
    State(st): State<AppState>,
    Json(req): Json<CreatePreset>,
) -> AppResult<Response> {
    let name = req.name.trim();
    if name.is_empty() {
        return Err(AppError(anyhow!("preset name is required")));
    }
    if name_taken(&st, &req.kind, name, None).await? {
        return Ok(conflict("a preset with that name already exists"));
    }

    let id = Uuid::new_v4().to_string();
    let data = serde_json::to_string(&req.data)?;
    sqlx::query("INSERT INTO presets (id, name, kind, data_json) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(name)
        .bind(&req.kind)
        .bind(&data)
        .execute(&st.pool)
        .await?;

    let row = fetch(&st, &id).await?;
    Ok((StatusCode::CREATED, Json(Preset::from(row))).into_response())
}

/// `PUT /api/presets/:id` — overwrite an existing preset ("Save").
pub async fn update(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdatePreset>,
) -> AppResult<Response> {
    let Some(existing) = fetch_opt(&st, &id).await? else {
        return Ok((StatusCode::NOT_FOUND, "preset not found").into_response());
    };

    let name = req.name.unwrap_or(existing.name);
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError(anyhow!("preset name is required")));
    }
    if name_taken(&st, &existing.kind, name, Some(&id)).await? {
        return Ok(conflict("a preset with that name already exists"));
    }
    let data = match req.data {
        Some(d) => serde_json::to_string(&d)?,
        None => existing.data_json,
    };

    sqlx::query(
        "UPDATE presets SET name = ?, data_json = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(name)
    .bind(&data)
    .bind(&id)
    .execute(&st.pool)
    .await?;

    let row = fetch(&st, &id).await?;
    Ok(Json(Preset::from(row)).into_response())
}

/// `DELETE /api/presets/:id`
pub async fn delete(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Json<serde_json::Value>> {
    sqlx::query("DELETE FROM presets WHERE id = ?")
        .bind(&id)
        .execute(&st.pool)
        .await?;
    Ok(Json(json!({ "deleted": id })))
}

async fn name_taken(
    st: &AppState,
    kind: &str,
    name: &str,
    exclude_id: Option<&str>,
) -> anyhow::Result<bool> {
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM presets WHERE kind = ? AND name = ?")
            .bind(kind)
            .bind(name)
            .fetch_optional(&st.pool)
            .await?;
    Ok(match (row, exclude_id) {
        (Some((found_id,)), Some(exclude)) => found_id != exclude,
        (Some(_), None) => true,
        (None, _) => false,
    })
}

async fn fetch(st: &AppState, id: &str) -> anyhow::Result<PresetRow> {
    Ok(sqlx::query_as::<_, PresetRow>("SELECT * FROM presets WHERE id = ?")
        .bind(id)
        .fetch_one(&st.pool)
        .await?)
}

async fn fetch_opt(st: &AppState, id: &str) -> anyhow::Result<Option<PresetRow>> {
    Ok(sqlx::query_as::<_, PresetRow>("SELECT * FROM presets WHERE id = ?")
        .bind(id)
        .fetch_optional(&st.pool)
        .await?)
}

fn conflict(msg: &str) -> Response {
    (StatusCode::CONFLICT, Json(json!({ "error": msg }))).into_response()
}

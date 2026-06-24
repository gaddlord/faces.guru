use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::str::FromStr;

/// Open (creating if needed) the SQLite pool and ensure the schema exists.
pub async fn init_pool(db_url: &str) -> Result<SqlitePool> {
    // Make sure the parent directory for the SQLite file exists.
    if let Some(path) = db_url.strip_prefix("sqlite://") {
        if let Some(parent) = std::path::Path::new(path).parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).ok();
            }
        }
    }

    let opts = SqliteConnectOptions::from_str(db_url)?.create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await?;

    migrate(&pool).await?;
    Ok(pool)
}

/// The only two tables in the system (§2.4 of the roadmap): `jobs` and `media`.
async fn migrate(pool: &SqlitePool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS jobs (
            id               TEXT PRIMARY KEY,
            job_type         TEXT NOT NULL,
            status           TEXT NOT NULL DEFAULT 'queued',
            params_json      TEXT NOT NULL DEFAULT '{}',
            input_media_ids  TEXT NOT NULL DEFAULT '[]',
            output_media_ids TEXT NOT NULL DEFAULT '[]',
            error            TEXT,
            progress         REAL NOT NULL DEFAULT 0,
            created_at       TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at       TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS media (
            id         TEXT PRIMARY KEY,
            path       TEXT NOT NULL,
            kind       TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

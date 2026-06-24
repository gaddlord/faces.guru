mod image;
mod swap;
mod video;

use std::time::Duration;

use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::models::Job;
use crate::AppState;

/// Background loop: claim one queued job at a time and run it to completion.
/// Serial execution keeps memory bounded (§2.2) — only one heavy model loaded at once.
pub async fn run(st: AppState) {
    tracing::info!("worker started (mock={})", st.cfg.mock);
    loop {
        match claim_next_job(&st).await {
            Ok(Some(job)) => {
                tracing::info!("processing job {} ({})", job.id, job.job_type);
                if let Err(e) = process(&st, &job).await {
                    tracing::error!("job {} failed: {:#}", job.id, e);
                    let _ = fail_job(&st, &job.id, &format!("{e:#}")).await;
                }
            }
            Ok(None) => tokio::time::sleep(Duration::from_millis(800)).await,
            Err(e) => {
                tracing::error!("worker poll error: {:#}", e);
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Atomically claim the oldest queued job (marks it running) using UPDATE ... RETURNING.
async fn claim_next_job(st: &AppState) -> Result<Option<Job>> {
    let job = sqlx::query_as::<_, Job>(
        "UPDATE jobs SET status = 'running', updated_at = datetime('now')
         WHERE id = (SELECT id FROM jobs WHERE status = 'queued' ORDER BY created_at LIMIT 1)
         RETURNING *",
    )
    .fetch_optional(&st.pool)
    .await?;
    Ok(job)
}

async fn process(st: &AppState, job: &Job) -> Result<()> {
    let params = job.params();
    let inputs = job.inputs();

    let outputs = match job.job_type.as_str() {
        "image" => image::run(st, job, &params).await?,
        "swap" => swap::run(st, job, &params, &inputs).await?,
        "video" => video::run(st, job, &params, &inputs).await?,
        other => return Err(anyhow!("unknown job type '{other}'")),
    };

    finish_job(st, &job.id, &outputs).await?;
    Ok(())
}

// ---- shared DB / media helpers used by the per-type processors ----

pub(crate) async fn set_progress(st: &AppState, job_id: &str, progress: f64) -> Result<()> {
    sqlx::query("UPDATE jobs SET progress = ?, updated_at = datetime('now') WHERE id = ?")
        .bind(progress)
        .bind(job_id)
        .execute(&st.pool)
        .await?;
    Ok(())
}

async fn finish_job(st: &AppState, job_id: &str, output_media_ids: &[String]) -> Result<()> {
    let outputs = serde_json::to_string(output_media_ids)?;
    sqlx::query(
        "UPDATE jobs SET status = 'done', progress = 1.0, output_media_ids = ?,
         updated_at = datetime('now') WHERE id = ?",
    )
    .bind(outputs)
    .bind(job_id)
    .execute(&st.pool)
    .await?;
    Ok(())
}

async fn fail_job(st: &AppState, job_id: &str, error: &str) -> Result<()> {
    sqlx::query(
        "UPDATE jobs SET status = 'failed', error = ?, updated_at = datetime('now') WHERE id = ?",
    )
    .bind(error)
    .bind(job_id)
    .execute(&st.pool)
    .await?;
    Ok(())
}

/// Persist raw bytes to the media dir + DB, returning the new media id.
pub(crate) async fn save_media(
    st: &AppState,
    kind: &str,
    ext: &str,
    bytes: &[u8],
) -> Result<String> {
    let id = Uuid::new_v4().to_string();
    tokio::fs::create_dir_all(&st.cfg.media_dir).await?;
    let path = format!("{}/{}.{}", st.cfg.media_dir, id, ext);
    tokio::fs::write(&path, bytes).await?;
    sqlx::query("INSERT INTO media (id, path, kind) VALUES (?, ?, ?)")
        .bind(&id)
        .bind(&path)
        .bind(kind)
        .execute(&st.pool)
        .await?;
    Ok(id)
}

/// Read the bytes of a previously stored media id.
pub(crate) async fn read_media(st: &AppState, media_id: &str) -> Result<Vec<u8>> {
    let row = sqlx::query_as::<_, (String,)>("SELECT path FROM media WHERE id = ?")
        .bind(media_id)
        .fetch_optional(&st.pool)
        .await?;
    let (path,) = row.ok_or_else(|| anyhow!("media id '{media_id}' not found"))?;
    Ok(tokio::fs::read(&path).await?)
}

/// Deterministic gradient placeholder PNG, seeded by the prompt — used in mock mode
/// so the whole pipeline produces real, viewable media without any model server.
pub(crate) fn placeholder_png(seed_text: &str, w: u32, h: u32) -> Result<Vec<u8>> {
    use ::image::{Rgb, RgbImage};

    // FNV-1a hash of the seed text → base color.
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in seed_text.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let r0 = (hash & 0xff) as f32;
    let g0 = ((hash >> 8) & 0xff) as f32;
    let b0 = ((hash >> 16) & 0xff) as f32;

    let mut img = RgbImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let fx = x as f32 / w as f32;
        let fy = y as f32 / h as f32;
        let r = (r0 * (1.0 - fx) + 255.0 * fx * fy).clamp(0.0, 255.0) as u8;
        let g = (g0 * (1.0 - fy) + 80.0 * fx).clamp(0.0, 255.0) as u8;
        let b = (b0 * (0.4 + 0.6 * fy)).clamp(0.0, 255.0) as u8;
        *p = Rgb([r, g, b]);
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, ::image::ImageFormat::Png)?;
    Ok(buf.into_inner())
}

/// Single-frame GIF placeholder used for mock video output (a real, viewable file).
pub(crate) fn placeholder_gif(seed_text: &str, w: u32, h: u32) -> Result<Vec<u8>> {
    use ::image::{Rgb, RgbImage};

    let mut hash: u64 = 0xcbf29ce484222325;
    for b in seed_text.bytes() {
        hash ^= b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    let base = ((hash >> 24) & 0xff) as f32;

    let mut img = RgbImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels_mut() {
        let fx = x as f32 / w as f32;
        let fy = y as f32 / h as f32;
        let r = (base * fx).clamp(0.0, 255.0) as u8;
        let g = (200.0 * fy).clamp(0.0, 255.0) as u8;
        let b = (150.0 * (1.0 - fx)).clamp(0.0, 255.0) as u8;
        *p = Rgb([r, g, b]);
    }

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, ::image::ImageFormat::Gif)?;
    Ok(buf.into_inner())
}

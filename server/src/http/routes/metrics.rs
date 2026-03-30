use actix_web::{get, web, HttpResponse};
use serde::Serialize;
use std::time::Instant;

use crate::app_state::AppState;
use crate::services::bandwidth::BandwidthManager;
use crate::services::upload_queue::UploadQueue;
use crate::storage;

#[derive(Serialize)]
struct MetricsResponse {
    uptime_secs: u64,
    cache_bytes: u64,
    cache_files: usize,
    bandwidth: BandwidthSnapshot,
    telegram_connected: bool,
    upload_queue_length: usize,
}

#[derive(Serialize)]
struct BandwidthSnapshot {
    date: String,
    up_bytes: u64,
    down_bytes: u64,
}

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn boot_instant() -> &'static Instant {
    START.get_or_init(Instant::now)
}

/// GET /api/metrics — operational metrics in JSON.
#[get("")]
async fn metrics(
    state: web::Data<AppState>,
    bw: web::Data<BandwidthManager>,
    queue: web::Data<UploadQueue>,
) -> HttpResponse {
    let telegram_connected = state.telegram_client.lock().await.is_some();
    let uptime_secs = boot_instant().elapsed().as_secs();
    let (cache_bytes, cache_files) = storage::cache::cache_stats(&state.cache_dir);
    let bw_stats = bw.get_stats();
    let upload_queue_length = queue.list_jobs().await.len();

    HttpResponse::Ok().json(MetricsResponse {
        uptime_secs,
        cache_bytes,
        cache_files,
        bandwidth: BandwidthSnapshot {
            date: bw_stats.date,
            up_bytes: bw_stats.up_bytes,
            down_bytes: bw_stats.down_bytes,
        },
        telegram_connected,
        upload_queue_length,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(metrics);
}

use actix_web::{get, web, HttpResponse};
use serde::Serialize;
use std::time::Instant;

use crate::app_state::AppState;
use crate::domain::models::AccountTier;
use crate::services::bandwidth::BandwidthManager;
use crate::services::upload_queue::UploadQueue;
use crate::storage;

#[derive(Serialize)]
struct MetricsResponse {
    uptime_secs: u64,
    cache_bytes: u64,
    cache_files: usize,
    max_file_size_bytes: u64,
    max_file_size_tier: AccountTier,
    dynamic_limits_enabled: bool,
    fallback_mode: bool,
    telegram_account_cached: bool,
    bandwidth: BandwidthSnapshot,
    telegram_connected: bool,
    upload_queue_length: usize,
}

#[derive(Serialize)]
struct BandwidthSnapshot {
    date: String,
    up_bytes: u64,
    down_bytes: u64,
    limit_bytes: u64,
    remaining_bytes: u64,
    tier: AccountTier,
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
    let tier = state.effective_tier().await;
    let max_file_size_bytes = state.effective_max_file_size_bytes().await;
    let bandwidth_limit_bytes = state.effective_daily_bandwidth_limit_bytes().await;
    let fallback_mode = state.dynamic_fallback_mode().await;
    let telegram_account_cached = state.telegram_account_profile().await.is_some();
    let bw_stats = bw.get_stats();
    let total_bandwidth_bytes = bw_stats.up_bytes + bw_stats.down_bytes;
    let remaining_bytes = bandwidth_limit_bytes.saturating_sub(total_bandwidth_bytes);
    let upload_queue_length = queue.list_jobs().await.len();

    HttpResponse::Ok().json(MetricsResponse {
        uptime_secs,
        cache_bytes,
        cache_files,
        max_file_size_bytes,
        max_file_size_tier: tier,
        dynamic_limits_enabled: state.dynamic_limits_enabled,
        fallback_mode,
        telegram_account_cached,
        bandwidth: BandwidthSnapshot {
            date: bw_stats.date,
            up_bytes: bw_stats.up_bytes,
            down_bytes: bw_stats.down_bytes,
            limit_bytes: bandwidth_limit_bytes,
            remaining_bytes,
            tier,
        },
        telegram_connected,
        upload_queue_length,
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(metrics);
}

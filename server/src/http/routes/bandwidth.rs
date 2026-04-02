use actix_web::{get, web, HttpResponse};
use serde::Serialize;

use crate::app_state::AppState;
use crate::domain::models::AccountTier;
use crate::errors::AppError;
use crate::services::bandwidth::BandwidthManager;

#[derive(Serialize)]
struct BandwidthResponse {
    date: String,
    up_bytes: u64,
    down_bytes: u64,
    limit_bytes: u64,
    remaining_bytes: u64,
    tier: AccountTier,
    dynamic_limits_enabled: bool,
    fallback_mode: bool,
}

/// GET /api/bandwidth
#[get("")]
async fn get_bandwidth(
    state: web::Data<AppState>,
    bw: web::Data<BandwidthManager>,
) -> Result<HttpResponse, AppError> {
    let tier = state.effective_tier().await;
    let limit_bytes = state.effective_daily_bandwidth_limit_bytes().await;
    let fallback_mode = state.dynamic_fallback_mode().await;
    let stats = bw.get_stats();
    let remaining_bytes = limit_bytes.saturating_sub(stats.up_bytes + stats.down_bytes);

    Ok(HttpResponse::Ok().json(BandwidthResponse {
        date: stats.date,
        up_bytes: stats.up_bytes,
        down_bytes: stats.down_bytes,
        limit_bytes,
        remaining_bytes,
        tier,
        dynamic_limits_enabled: state.dynamic_limits_enabled,
        fallback_mode,
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_bandwidth);
}

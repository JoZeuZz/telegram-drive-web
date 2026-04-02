use actix_web::{get, web, HttpResponse};
use serde::Serialize;

use crate::app_state::AppState;
use crate::domain::models::{AccountTier, TelegramAccountProfile};
use crate::services::bandwidth::BandwidthManager;

#[derive(Serialize)]
struct AccountInfoResponse {
    authenticated: bool,
    dynamic_limits_enabled: bool,
    fallback_mode: bool,
    tier: AccountTier,
    limits: AccountLimits,
    bandwidth: BandwidthSnapshot,
    profile: Option<AccountProfileResponse>,
}

#[derive(Serialize)]
struct AccountLimits {
    file_size_limit_bytes: u64,
    daily_bandwidth_limit_bytes: u64,
}

#[derive(Serialize)]
struct BandwidthSnapshot {
    date: String,
    up_bytes: u64,
    down_bytes: u64,
    limit_bytes: u64,
    remaining_bytes: u64,
}

#[derive(Serialize)]
struct AccountProfileResponse {
    user_id: i64,
    first_name: Option<String>,
    last_name: Option<String>,
    username: Option<String>,
    phone: Option<String>,
    is_premium: bool,
    checked_at_unix_ms: i64,
}

fn map_profile(profile: TelegramAccountProfile) -> AccountProfileResponse {
    AccountProfileResponse {
        user_id: profile.user_id,
        first_name: profile.first_name,
        last_name: profile.last_name,
        username: profile.username,
        phone: profile.phone,
        is_premium: profile.is_premium,
        checked_at_unix_ms: profile.checked_at_unix_ms,
    }
}

/// GET /api/account-info
#[get("")]
async fn account_info(state: web::Data<AppState>, bw: web::Data<BandwidthManager>) -> HttpResponse {
    let tier = state.effective_tier().await;
    let file_size_limit_bytes = state.effective_max_file_size_bytes().await;
    let bandwidth_limit_bytes = state.effective_daily_bandwidth_limit_bytes().await;
    let fallback_mode = state.dynamic_fallback_mode().await;
    let profile = state.telegram_account_profile().await;

    let stats = bw.get_stats();
    let total = stats.up_bytes + stats.down_bytes;
    let remaining_bytes = bandwidth_limit_bytes.saturating_sub(total);

    HttpResponse::Ok().json(AccountInfoResponse {
        authenticated: true,
        dynamic_limits_enabled: state.dynamic_limits_enabled,
        fallback_mode,
        tier,
        limits: AccountLimits {
            file_size_limit_bytes,
            daily_bandwidth_limit_bytes: bandwidth_limit_bytes,
        },
        bandwidth: BandwidthSnapshot {
            date: stats.date,
            up_bytes: stats.up_bytes,
            down_bytes: stats.down_bytes,
            limit_bytes: bandwidth_limit_bytes,
            remaining_bytes,
        },
        profile: profile.map(map_profile),
    })
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(account_info);
}

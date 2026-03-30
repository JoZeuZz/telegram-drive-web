use actix_web::{get, web, HttpResponse, Responder};
use serde::Serialize;
use std::time::Instant;

use crate::app_state::AppState;
use crate::storage;

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    telegram_connected: bool,
    uptime_secs: u64,
    cache_bytes: u64,
}

#[derive(Serialize)]
struct VersionResponse {
    name: &'static str,
    version: &'static str,
}

static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();

fn boot_instant() -> &'static Instant {
    START.get_or_init(Instant::now)
}

/// GET /api/health — comprehensive health check
#[get("/health")]
pub async fn health_check(state: web::Data<AppState>) -> impl Responder {
    let telegram_connected = state.telegram_client.lock().await.is_some();
    let uptime_secs = boot_instant().elapsed().as_secs();
    let cache_bytes = storage::cache::cache_size_bytes(&state.cache_dir);

    HttpResponse::Ok().json(HealthResponse {
        status: "ok",
        telegram_connected,
        uptime_secs,
        cache_bytes,
    })
}

/// GET /api/version — server version info
#[get("/version")]
pub async fn version_info() -> impl Responder {
    HttpResponse::Ok().json(VersionResponse {
        name: "telegram-drive-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

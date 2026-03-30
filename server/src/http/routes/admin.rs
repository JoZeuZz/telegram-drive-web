use actix_web::{post, web, HttpResponse};

use crate::app_state::AppState;
use crate::errors::AppError;
use crate::jobs;

/// POST /api/admin/clean-cache — manually trigger cache cleanup.
#[post("/clean-cache")]
async fn clean_cache(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let (files_removed, bytes_freed) = jobs::cleanup::cleanup_cache(&state.cache_dir);
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "files_removed": files_removed,
        "bytes_freed": bytes_freed,
    })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(clean_cache);
}

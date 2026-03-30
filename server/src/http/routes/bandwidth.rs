use actix_web::{get, web, HttpResponse};

use crate::errors::AppError;
use crate::services::bandwidth::BandwidthManager;

/// GET /api/bandwidth
#[get("")]
async fn get_bandwidth(bw: web::Data<BandwidthManager>) -> Result<HttpResponse, AppError> {
    let stats = bw.get_stats();
    Ok(HttpResponse::Ok().json(stats))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(get_bandwidth);
}

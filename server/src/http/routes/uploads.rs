use actix_web::{delete, get, post, web, HttpResponse};

use crate::domain::dto::SuccessResponse;
use crate::errors::AppError;
use crate::services::upload_queue::UploadQueue;

/// GET /api/uploads
#[get("")]
async fn list_uploads(queue: web::Data<UploadQueue>) -> Result<HttpResponse, AppError> {
    let jobs = queue.list_jobs().await;
    Ok(HttpResponse::Ok().json(jobs))
}

/// POST /api/uploads/{id}/cancel
#[post("/{id}/cancel")]
async fn cancel_upload(
    path: web::Path<String>,
    queue: web::Data<UploadQueue>,
) -> Result<HttpResponse, AppError> {
    let id = path.into_inner();
    let cancelled = queue.cancel_job(&id).await;
    if cancelled {
        Ok(HttpResponse::Ok().json(SuccessResponse { success: true }))
    } else {
        Err(AppError::BadRequest(
            "Job not found or already processing".into(),
        ))
    }
}

/// DELETE /api/uploads/finished
#[delete("/finished")]
async fn clear_finished(queue: web::Data<UploadQueue>) -> Result<HttpResponse, AppError> {
    let removed = queue.clear_finished().await;
    Ok(HttpResponse::Ok().json(serde_json::json!({
        "removed": removed,
    })))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_uploads)
        .service(clear_finished)
        .service(cancel_upload);
}

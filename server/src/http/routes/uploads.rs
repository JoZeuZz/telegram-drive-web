use actix_web::{delete, get, post, web, HttpResponse};

use crate::domain::dto::SuccessResponse;
use crate::errors::AppError;
use crate::services::upload_progress::UploadProgressManager;
use crate::services::upload_queue::UploadQueue;

/// GET /api/uploads
#[get("")]
async fn list_uploads(queue: web::Data<UploadQueue>) -> Result<HttpResponse, AppError> {
    let jobs = queue.list_jobs().await;
    Ok(HttpResponse::Ok().json(jobs))
}

/// GET /api/uploads/{id}
#[get("/{id}")]
async fn get_upload_progress(
    path: web::Path<String>,
    progress: web::Data<UploadProgressManager>,
) -> Result<HttpResponse, AppError> {
    let upload_id = path.into_inner();
    let Some(snapshot) = progress.snapshot(&upload_id) else {
        return Err(AppError::NotFound("Upload progress not found".into()));
    };

    Ok(HttpResponse::Ok().json(snapshot))
}

/// GET /api/uploads/{id}/events
#[get("/{id}/events")]
async fn stream_upload_progress(
    path: web::Path<String>,
    progress: web::Data<UploadProgressManager>,
) -> Result<HttpResponse, AppError> {
    let upload_id = path.into_inner();
    let Some(initial_snapshot) = progress.snapshot(&upload_id) else {
        return Err(AppError::NotFound("Upload progress not found".into()));
    };
    let Some(mut rx) = progress.subscribe(&upload_id) else {
        return Err(AppError::NotFound("Upload progress not found".into()));
    };

    let stream = async_stream::stream! {
        let initial_payload = match serde_json::to_string(&initial_snapshot) {
            Ok(payload) => payload,
            Err(_) => "{}".to_string(),
        };
        let initial_event = format!("data: {}\n\n", initial_payload);
        yield Ok::<actix_web::web::Bytes, actix_web::Error>(actix_web::web::Bytes::from(initial_event));

        loop {
            match rx.recv().await {
                Ok(snapshot) => {
                    let payload = match serde_json::to_string(&snapshot) {
                        Ok(payload) => payload,
                        Err(_) => continue,
                    };
                    let event = format!("data: {}\n\n", payload);
                    yield Ok(actix_web::web::Bytes::from(event));
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    Ok(HttpResponse::Ok()
        .insert_header(("Content-Type", "text/event-stream"))
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .streaming(stream))
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
    .service(get_upload_progress)
    .service(stream_upload_progress)
        .service(clear_finished)
        .service(cancel_upload);
}

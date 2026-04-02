use actix_multipart::Multipart;
use actix_web::{delete, get, post, web, HttpResponse};
use futures::{StreamExt, TryStreamExt};
use std::io::Write;

use crate::app_state::AppState;
use crate::domain::dto::*;
use crate::errors::AppError;
use crate::services::{
    bandwidth::BandwidthManager,
    streaming, telegram_files,
    upload_progress::{UploadProgressManager, UploadProgressReporter},
    upload_queue::{UploadJob, UploadQueue},
};

/// GET /api/files?folder_id=
#[get("")]
async fn list_files(
    query: web::Query<FolderIdQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let files = telegram_files::get_files(
        &state,
        query.folder_id,
        query.topic_id,
        query.topic_top_message,
    )
    .await?;
    Ok(HttpResponse::Ok().json(files))
}

/// DELETE /api/files/{message_id}?folder_id=
#[delete("/{message_id}")]
async fn delete_file(
    path: web::Path<i32>,
    query: web::Query<FolderIdQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let message_id = path.into_inner();
    telegram_files::delete_file(&state, message_id, query.folder_id).await?;
    Ok(HttpResponse::Ok().json(SuccessResponse { success: true }))
}

/// POST /api/files/move
#[post("/move")]
async fn move_files(
    body: web::Json<MoveFilesRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    telegram_files::move_files(
        &state,
        &body.message_ids,
        body.source_folder_id,
        body.source_topic_id,
        body.target_folder_id,
        body.target_topic_id,
        body.target_topic_top_message,
    )
    .await?;
    Ok(HttpResponse::Ok().json(SuccessResponse { success: true }))
}

/// POST /api/files/upload?folder_id=&queue=false
#[post("/upload")]
async fn upload_file(
    mut payload: Multipart,
    query: web::Query<UploadQuery>,
    state: web::Data<AppState>,
    bw: web::Data<BandwidthManager>,
    queue: web::Data<UploadQueue>,
    upload_progress: web::Data<UploadProgressManager>,
) -> Result<HttpResponse, AppError> {
    let upload_dir = std::path::Path::new(&state.cache_dir).join("uploads");
    std::fs::create_dir_all(&upload_dir)
        .map_err(|e| AppError::Internal(format!("Cannot create upload dir: {}", e)))?;

    let progress_upload_id = if query.queue {
        None
    } else {
        query
            .upload_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    let file_size_limit_bytes = state.effective_max_file_size_bytes().await;

    while let Some(mut field) = match payload.try_next().await {
        Ok(field) => field,
        Err(error) => {
            if let Some(upload_id) = progress_upload_id.as_deref() {
                upload_progress.mark_failed(upload_id, format!("Multipart error: {}", error));
            }
            return Err(AppError::BadRequest(format!("Multipart error: {}", error)));
        }
    } {
        let content_type = field.content_type().map(|mime| mime.to_string());

        // Sanitize filename to prevent path traversal
        let raw_name = field
            .content_disposition()
            .and_then(|cd| cd.get_filename().map(|s| s.to_string()))
            .unwrap_or_else(|| "unnamed".to_string());
        let filename = std::path::Path::new(&raw_name)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unnamed".to_string());

        if let Some(upload_id) = progress_upload_id.as_deref() {
            upload_progress.start_upload(
                upload_id,
                &filename,
                query.upload_size_bytes.unwrap_or(0),
            );
        }

        let temp_path = upload_dir.join(format!("upload_{}", uuid::Uuid::new_v4()));
        let temp_path_str = temp_path.to_string_lossy().to_string();

        // Stream chunks to temp file
        let size = {
            let mut total_bytes = 0u64;
            let mut f = std::fs::File::create(&temp_path)
                .map_err(|e| AppError::Internal(format!("Cannot create temp file: {}", e)))?;
            while let Some(chunk) = match field.try_next().await {
                Ok(chunk) => chunk,
                Err(error) => {
                    drop(f);
                    let _ = std::fs::remove_file(&temp_path);
                    if let Some(upload_id) = progress_upload_id.as_deref() {
                        upload_progress.mark_failed(upload_id, format!("Read error: {}", error));
                    }
                    return Err(AppError::BadRequest(format!("Read error: {}", error)));
                }
            } {
                total_bytes = total_bytes.saturating_add(chunk.len() as u64);
                if let Some(upload_id) = progress_upload_id.as_deref() {
                    upload_progress.update_browser_bytes(upload_id, total_bytes);
                }
                if total_bytes > file_size_limit_bytes {
                    drop(f);
                    let _ = std::fs::remove_file(&temp_path);
                    if let Some(upload_id) = progress_upload_id.as_deref() {
                        upload_progress.mark_failed(
                            upload_id,
                            format!(
                                "File exceeds maximum allowed size ({} bytes)",
                                file_size_limit_bytes
                            ),
                        );
                    }
                    return Err(AppError::BadRequest(format!(
                        "File exceeds maximum allowed size ({} bytes)",
                        file_size_limit_bytes
                    )));
                }
                f.write_all(&chunk)
                    .map_err(|e| AppError::Internal(format!("Write error: {}", e)))?;
            }
            total_bytes
        };

        if let Some(upload_id) = progress_upload_id.as_deref() {
            upload_progress.set_file_size(upload_id, size);
            upload_progress.switch_to_telegram_stage(upload_id);
        }

        if query.queue {
            let job_id = uuid::Uuid::new_v4().to_string();
            queue
                .enqueue(UploadJob {
                    id: job_id.clone(),
                    file_path: temp_path_str,
                    file_name: filename,
                    content_type,
                    folder_id: query.folder_id,
                    topic_id: query.topic_id,
                    topic_top_message: query.topic_top_message,
                    size,
                    as_photo: query.as_photo,
                })
                .await?;
            return Ok(HttpResponse::Accepted().json(UploadEnqueuedResponse {
                id: job_id,
                status: "queued".into(),
            }));
        } else {
            let progress_reporter = progress_upload_id.as_ref().map(|upload_id| {
                UploadProgressReporter::new(upload_progress.get_ref().clone(), upload_id.clone())
            });

            let result = telegram_files::upload_file(
                &state,
                &bw,
                &temp_path_str,
                query.folder_id,
                query.topic_id,
                query.topic_top_message,
                &filename,
                content_type.as_deref(),
                query.as_photo,
                progress_reporter,
            )
            .await;

            if let Some(upload_id) = progress_upload_id.as_deref() {
                match &result {
                    Ok(_) => upload_progress.mark_completed(upload_id),
                    Err(error) => upload_progress.mark_failed(upload_id, error.to_string()),
                }
            }

            let _ = std::fs::remove_file(&temp_path);
            result?;
            return Ok(HttpResponse::Ok().json(MessageResponse {
                message: format!("Uploaded: {}", filename),
            }));
        }
    }

    Err(AppError::BadRequest("No file in request".into()))
}

/// GET /api/files/{message_id}/download?folder_id=
#[get("/{message_id}/download")]
async fn download_file(
    path: web::Path<i32>,
    query: web::Query<FolderIdQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let message_id = path.into_inner();
    let (info, stream) = streaming::prepare_stream(&state, query.folder_id, message_id).await?;

    let byte_stream = stream.map(|chunk| {
        chunk
            .map(actix_web::web::Bytes::from)
            .map_err(actix_web::error::ErrorInternalServerError)
    });

    let disposition = format!(
        "attachment; filename=\"{}\"",
        info.file_name.replace('"', "_")
    );

    Ok(HttpResponse::Ok()
        .content_type(info.mime_type)
        .insert_header(("Content-Disposition", disposition))
        .insert_header(("Content-Length", info.size.to_string()))
        .streaming(byte_stream))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_files)
        .service(upload_file)
        .service(move_files)
        .service(download_file)
        .service(delete_file);
}

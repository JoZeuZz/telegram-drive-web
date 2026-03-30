use actix_web::{get, web, HttpResponse};
use base64::{engine::general_purpose, Engine as _};
use futures::StreamExt;

use crate::app_state::AppState;
use crate::domain::dto::FolderIdQuery;
use crate::errors::AppError;
use crate::services::{bandwidth::BandwidthManager, previews, streaming};

/// GET /api/media/stream/{message_id}?folder_id=
#[get("/stream/{message_id}")]
async fn stream_media(
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

    Ok(HttpResponse::Ok()
        .content_type(info.mime_type)
        .insert_header(("Content-Length", info.size.to_string()))
        .streaming(byte_stream))
}

/// GET /api/media/preview/{message_id}?folder_id=
#[get("/preview/{message_id}")]
async fn preview(
    path: web::Path<i32>,
    query: web::Query<FolderIdQuery>,
    state: web::Data<AppState>,
    bw: web::Data<BandwidthManager>,
) -> Result<HttpResponse, AppError> {
    let message_id = path.into_inner();
    let result = previews::get_preview(&state, &bw, message_id, query.folder_id).await?;
    serve_preview_result(&result)
}

/// GET /api/media/thumbnail/{message_id}?folder_id=
#[get("/thumbnail/{message_id}")]
async fn thumbnail(
    path: web::Path<i32>,
    query: web::Query<FolderIdQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let message_id = path.into_inner();
    let result = previews::get_thumbnail(&state, message_id, query.folder_id).await?;
    if result.is_empty() {
        return Err(AppError::NotFound("No thumbnail available".into()));
    }
    serve_preview_result(&result)
}

/// Convert a preview/thumbnail result to an HTTP response.
/// Handles both data URLs (`data:mime;base64,...`) and file paths.
fn serve_preview_result(result: &str) -> Result<HttpResponse, AppError> {
    if let Some(rest) = result.strip_prefix("data:") {
        if let Some(semi) = rest.find(';') {
            let mime = &rest[..semi];
            if let Some(data_start) = rest.find(',') {
                let b64 = &rest[data_start + 1..];
                let bytes = general_purpose::STANDARD
                    .decode(b64)
                    .map_err(|e| AppError::Internal(format!("Base64 decode error: {}", e)))?;
                return Ok(HttpResponse::Ok()
                    .content_type(mime.to_string())
                    .insert_header(("Cache-Control", "private, max-age=3600"))
                    .body(bytes));
            }
        }
        Err(AppError::Internal("Invalid data URL format".into()))
    } else {
        let path = std::path::Path::new(result);
        let bytes = std::fs::read(path)
            .map_err(|e| AppError::Internal(format!("Cannot read preview file: {}", e)))?;
        let mime = mime_guess::from_path(path)
            .first_or_octet_stream()
            .to_string();
        Ok(HttpResponse::Ok()
            .content_type(mime)
            .insert_header(("Cache-Control", "private, max-age=3600"))
            .body(bytes))
    }
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(stream_media)
        .service(preview)
        .service(thumbnail);
}

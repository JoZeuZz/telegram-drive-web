use actix_web::{delete, get, post, web, HttpResponse};

use crate::app_state::AppState;
use crate::domain::dto::*;
use crate::errors::AppError;
use crate::services::telegram_folders;

/// GET /api/folders
#[get("")]
async fn list_folders(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let folders = telegram_folders::scan_folders(&state).await?;
    Ok(HttpResponse::Ok().json(folders))
}

/// POST /api/folders
#[post("")]
async fn create_folder(
    body: web::Json<CreateFolderRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let folder = telegram_folders::create_folder(&state, &body.name, body.parent_id).await?;
    Ok(HttpResponse::Created().json(folder))
}

/// DELETE /api/folders/{folder_id}
#[delete("/{folder_id}")]
async fn delete_folder(
    path: web::Path<i64>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let folder_id = path.into_inner();
    let deleted_count = telegram_folders::delete_folder(&state, folder_id).await?;
    Ok(HttpResponse::Ok().json(DeleteFolderResponse {
        success: true,
        deleted_count,
    }))
}

/// POST /api/folders/sync — force rescan of Telegram folders.
#[post("/sync")]
async fn sync_folders(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let report = telegram_folders::scan_folders_with_report(&state).await?;
    Ok(HttpResponse::Ok().json(FolderSyncResponse {
        folders: report.folders,
        summary: FolderSyncSummaryResponse {
            resolved_by_title: report.resolved_by_title,
            resolved_by_about: report.resolved_by_about,
            orphans: report.orphans,
            migrated: report.migrated,
        },
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_folders)
        .service(create_folder)
        .service(sync_folders)
        .service(delete_folder);
}

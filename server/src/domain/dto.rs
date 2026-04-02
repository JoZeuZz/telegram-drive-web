// DTOs — Data Transfer Objects for HTTP request/response payloads.
use crate::domain::models::FolderMetadata;
use serde::{Deserialize, Serialize};

// ─── App Auth ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub success: bool,
}

#[derive(Serialize)]
pub struct AuthStatusResponse {
    pub authenticated: bool,
}

// ─── Telegram Auth ───────────────────────────────────────

#[derive(Deserialize)]
pub struct TelegramConnectRequest {
    pub api_id: i32,
}

#[derive(Deserialize)]
pub struct TelegramRequestCodeRequest {
    pub phone: String,
    pub api_id: i32,
    pub api_hash: String,
}

#[derive(Deserialize)]
pub struct TelegramSignInRequest {
    pub code: String,
}

#[derive(Deserialize)]
pub struct TelegramCheckPasswordRequest {
    pub password: String,
}

#[derive(Serialize)]
pub struct TelegramStatusResponse {
    pub connected: bool,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub message: String,
}

// ─── Files ───────────────────────────────────────────────

#[derive(Deserialize)]
pub struct FolderIdQuery {
    pub folder_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct MoveFilesRequest {
    pub message_ids: Vec<i32>,
    pub source_folder_id: Option<i64>,
    pub target_folder_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UploadQuery {
    pub folder_id: Option<i64>,
    #[serde(default)]
    pub queue: bool,
    #[serde(default)]
    pub as_photo: bool,
    #[serde(default)]
    pub upload_id: Option<String>,
    #[serde(default)]
    pub upload_size_bytes: Option<u64>,
}

// ─── Folders ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
    #[serde(default)]
    pub parent_id: Option<i64>,
}

#[derive(Serialize)]
pub struct DeleteFolderResponse {
    pub success: bool,
    pub deleted_count: usize,
}

#[derive(Serialize)]
pub struct FolderSyncSummaryResponse {
    pub resolved_by_title: usize,
    pub resolved_by_about: usize,
    pub orphans: usize,
    pub migrated: usize,
}

#[derive(Serialize)]
pub struct FolderSyncResponse {
    pub folders: Vec<FolderMetadata>,
    pub summary: FolderSyncSummaryResponse,
}

// ─── Search ──────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SearchQuery {
    pub q: String,
}

// ─── Generic Responses ───────────────────────────────────

#[derive(Serialize)]
pub struct SuccessResponse {
    pub success: bool,
}

#[derive(Serialize)]
pub struct UploadEnqueuedResponse {
    pub id: String,
    pub status: String,
}

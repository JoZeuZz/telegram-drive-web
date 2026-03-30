// DTOs — Data Transfer Objects for HTTP request/response payloads.
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
}

// ─── Folders ─────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateFolderRequest {
    pub name: String,
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

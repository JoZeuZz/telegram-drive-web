use serde::{Deserialize, Serialize};

/// Represents the state machine for Telegram authentication flow.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "status", content = "data")]
pub enum AuthState {
    LoggedOut,
    AwaitingCode {
        phone: String,
        phone_code_hash: String,
    },
    AwaitingPassword {
        phone: String,
    },
    LoggedIn,
}

/// Result of an authentication step.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthResult {
    pub success: bool,
    /// Next step in the flow: "code", "password", or "dashboard"
    pub next_step: Option<String>,
    pub error: Option<String>,
}

/// Metadata for a file stored in Telegram.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileMetadata {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub name: String,
    pub size: u64,
    pub mime_type: Option<String>,
    pub file_ext: Option<String>,
    pub created_at: String,
    pub icon_type: String,
}

/// Metadata for a folder (Telegram private channel).
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FolderMetadata {
    pub id: i64,
    pub parent_id: Option<i64>,
    pub name: String,
}

/// Daily bandwidth usage statistics.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BandwidthStats {
    pub date: String,
    pub up_bytes: u64,
    pub down_bytes: u64,
}

impl Default for BandwidthStats {
    fn default() -> Self {
        Self {
            date: chrono::Local::now().format("%Y-%m-%d").to_string(),
            up_bytes: 0,
            down_bytes: 0,
        }
    }
}

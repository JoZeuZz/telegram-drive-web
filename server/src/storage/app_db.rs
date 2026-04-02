use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::domain::models::TelegramAccountProfile;

#[derive(Serialize, Deserialize)]
struct AdminFile {
    password_hash: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StructuredFolderCacheEntry {
    pub id: i64,
    pub name: String,
    pub access_hash: Option<i64>,
}

/// Path to the admin credentials file.
pub fn admin_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("admin.json")
}

/// Path to the cached Telegram account profile.
pub fn telegram_account_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("telegram_account.json")
}

/// Path to structured folders cache file.
pub fn structured_folders_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("structured_folders.json")
}

/// Load the stored admin password hash, if it exists.
pub fn load_admin_hash(data_dir: &str) -> Option<String> {
    let path = admin_path(data_dir);
    let content = std::fs::read_to_string(&path).ok()?;
    let admin: AdminFile = serde_json::from_str(&content).ok()?;
    Some(admin.password_hash)
}

/// Persist the admin password hash to disk.
pub fn save_admin_hash(data_dir: &str, hash: &str) -> Result<(), crate::errors::AppError> {
    let path = admin_path(data_dir);
    let admin = AdminFile {
        password_hash: hash.to_string(),
    };
    let json = serde_json::to_string_pretty(&admin)
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| {
        crate::errors::AppError::Internal(format!("Cannot write admin.json: {}", e))
    })?;
    Ok(())
}

/// Load cached Telegram account profile from disk, if it exists.
pub fn load_telegram_account(data_dir: &str) -> Option<TelegramAccountProfile> {
    let path = telegram_account_path(data_dir);
    let content = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Persist the Telegram account profile to disk.
pub fn save_telegram_account(
    data_dir: &str,
    profile: &TelegramAccountProfile,
) -> Result<(), crate::errors::AppError> {
    let path = telegram_account_path(data_dir);
    let json = serde_json::to_string_pretty(profile)
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| {
        crate::errors::AppError::Internal(format!("Cannot write telegram_account.json: {}", e))
    })?;
    Ok(())
}

/// Remove cached Telegram account profile from disk.
pub fn clear_telegram_account(data_dir: &str) {
    let path = telegram_account_path(data_dir);
    let _ = std::fs::remove_file(path);
}

/// Load structured folders cache from disk.
pub fn load_structured_folders(data_dir: &str) -> Vec<StructuredFolderCacheEntry> {
    let path = structured_folders_path(data_dir);
    let content = match std::fs::read_to_string(path) {
        Ok(content) => content,
        Err(_) => return Vec::new(),
    };

    serde_json::from_str::<Vec<StructuredFolderCacheEntry>>(&content).unwrap_or_default()
}

/// Persist full structured folders cache to disk.
pub fn save_structured_folders(
    data_dir: &str,
    entries: &[StructuredFolderCacheEntry],
) -> Result<(), crate::errors::AppError> {
    let path = structured_folders_path(data_dir);
    let json = serde_json::to_string_pretty(entries)
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?;
    std::fs::write(&path, json).map_err(|e| {
        crate::errors::AppError::Internal(format!("Cannot write structured_folders.json: {}", e))
    })?;
    Ok(())
}

/// Insert or update a structured folder entry in cache.
pub fn upsert_structured_folder(
    data_dir: &str,
    entry: StructuredFolderCacheEntry,
) -> Result<(), crate::errors::AppError> {
    let mut entries = load_structured_folders(data_dir);

    if let Some(existing) = entries.iter_mut().find(|candidate| candidate.id == entry.id) {
        existing.name = entry.name;
        existing.access_hash = entry.access_hash.or(existing.access_hash);
    } else {
        entries.push(entry);
    }

    save_structured_folders(data_dir, &entries)
}

/// Remove a structured folder cache entry by id.
pub fn remove_structured_folder(
    data_dir: &str,
    folder_id: i64,
) -> Result<(), crate::errors::AppError> {
    let mut entries = load_structured_folders(data_dir);
    entries.retain(|entry| entry.id != folder_id);
    save_structured_folders(data_dir, &entries)
}

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct AdminFile {
    password_hash: String,
}

/// Path to the admin credentials file.
pub fn admin_path(data_dir: &str) -> PathBuf {
    Path::new(data_dir).join("admin.json")
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

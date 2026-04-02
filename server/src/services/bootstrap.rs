use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};

use crate::storage;

/// Hash a password with Argon2id.
pub fn hash_password(password: &str) -> Result<String, crate::errors::AppError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| {
            crate::errors::AppError::Internal(format!("Failed to hash password: {}", e))
        })?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against an Argon2 hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    let parsed = match PasswordHash::new(hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Ensure the admin hash file exists in `data_dir`.
/// If missing, hash `default_password` and persist it.
/// Returns the stored hash.
pub fn ensure_admin(
    data_dir: &str,
    default_password: &str,
) -> Result<String, crate::errors::AppError> {
    if let Some(hash) = storage::app_db::load_admin_hash(data_dir) {
        tracing::info!(
            "Admin hash loaded from {}",
            storage::app_db::admin_path(data_dir).display()
        );
        return Ok(hash);
    }

    tracing::info!("No admin.json found — bootstrapping admin user");
    let hashed = hash_password(default_password)?;
    storage::app_db::save_admin_hash(data_dir, &hashed)?;
    tracing::info!(
        "Admin hash written to {}",
        storage::app_db::admin_path(data_dir).display()
    );
    Ok(hashed)
}

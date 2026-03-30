use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use tokio::sync::{Mutex, RwLock};
use grammers_client::Client;
use grammers_client::types::{LoginToken, PasswordToken};

use crate::config::Config;

/// Global application state shared across all request handlers.
pub struct AppState {
    /// Telegram client (None until authenticated)
    pub telegram_client: Arc<Mutex<Option<Client>>>,
    /// Login token for the sign-in flow
    pub login_token: Arc<Mutex<Option<LoginToken>>>,
    /// Password token when 2FA is required
    pub password_token: Arc<Mutex<Option<PasswordToken>>>,
    /// Stored API ID for auto-reconnect
    pub api_id: Arc<Mutex<Option<i32>>>,
    /// Send to this channel to request runner shutdown
    pub runner_shutdown: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
    /// Counter for debugging runner lifecycle
    pub runner_count: Arc<AtomicU32>,
    /// Path to persistent data directory
    pub data_dir: String,
    /// Path to cache directory
    pub cache_dir: String,
    /// Session secret for cookie signing
    pub session_secret: String,
    /// Telegram API ID from config
    pub config_api_id: i32,
    /// Telegram API hash from config
    pub config_api_hash: String,
    /// Argon2 hash of admin password (mutable for password change)
    pub admin_password_hash: RwLock<String>,
}

impl AppState {
    pub fn new(config: &Config, admin_password_hash: String) -> Self {
        // Ensure data directory exists
        if let Err(e) = std::fs::create_dir_all(&config.data_dir) {
            tracing::error!("Failed to create data directory: {}", e);
        }
        // Ensure cache directory exists
        if let Err(e) = std::fs::create_dir_all(&config.cache_dir) {
            tracing::error!("Failed to create cache directory: {}", e);
        }

        Self {
            telegram_client: Arc::new(Mutex::new(None)),
            login_token: Arc::new(Mutex::new(None)),
            password_token: Arc::new(Mutex::new(None)),
            api_id: Arc::new(Mutex::new(None)),
            runner_shutdown: Arc::new(Mutex::new(None)),
            runner_count: Arc::new(AtomicU32::new(0)),
            data_dir: config.data_dir.clone(),
            cache_dir: config.cache_dir.clone(),
            session_secret: config.session_secret.clone(),
            config_api_id: config.telegram_api_id,
            config_api_hash: config.telegram_api_hash.clone(),
            admin_password_hash: RwLock::new(admin_password_hash),
        }
    }
}

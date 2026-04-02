use grammers_client::types::{LoginToken, PasswordToken};
use grammers_client::Client;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

use crate::config::Config;
use crate::domain::models::{AccountTier, TelegramAccountProfile};

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
    /// Maximum upload size allowed per file (bytes)
    pub max_file_size_bytes: u64,
    /// Maximum upload size allowed for premium accounts.
    pub premium_max_file_size_bytes: u64,
    /// Daily bandwidth limit for free accounts.
    pub free_daily_bandwidth_limit_bytes: u64,
    /// Daily bandwidth limit for premium accounts.
    pub premium_daily_bandwidth_limit_bytes: u64,
    /// Toggle for tier-aware dynamic limits.
    pub dynamic_limits_enabled: bool,
    /// Toggle for forum/community endpoints and services.
    pub forums_enabled: bool,
    /// If premium detection is stale/missing, fallback to free tier.
    pub fallback_to_free_on_error: bool,
    /// Max age for cached account profile before being considered stale.
    pub premium_detection_ttl_secs: u64,
    /// Cached account profile from Telegram login/check_connection.
    pub telegram_account: Arc<RwLock<Option<TelegramAccountProfile>>>,
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

        let cached_account = crate::storage::app_db::load_telegram_account(&config.data_dir);

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
            max_file_size_bytes: config.max_file_size_bytes,
            premium_max_file_size_bytes: config.premium_max_file_size_bytes,
            free_daily_bandwidth_limit_bytes: config.free_daily_bandwidth_limit_bytes,
            premium_daily_bandwidth_limit_bytes: config.premium_daily_bandwidth_limit_bytes,
            dynamic_limits_enabled: config.dynamic_limits_enabled,
            forums_enabled: config.forums_enabled,
            fallback_to_free_on_error: config.fallback_to_free_on_error,
            premium_detection_ttl_secs: config.premium_detection_ttl_secs,
            telegram_account: Arc::new(RwLock::new(cached_account)),
            config_api_id: config.telegram_api_id,
            config_api_hash: config.telegram_api_hash.clone(),
            admin_password_hash: RwLock::new(admin_password_hash),
        }
    }

    fn profile_is_fresh(&self, profile: &TelegramAccountProfile) -> bool {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let max_age_ms = i64::try_from(self.premium_detection_ttl_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        now_ms.saturating_sub(profile.checked_at_unix_ms) <= max_age_ms
    }

    pub async fn telegram_account_profile(&self) -> Option<TelegramAccountProfile> {
        self.telegram_account.read().await.clone()
    }

    pub async fn set_telegram_account_profile(&self, profile: TelegramAccountProfile) {
        {
            let mut guard = self.telegram_account.write().await;
            *guard = Some(profile.clone());
        }

        if let Err(err) = crate::storage::app_db::save_telegram_account(&self.data_dir, &profile) {
            tracing::warn!(error = %err, "Failed to persist telegram account profile cache");
        }
    }

    pub async fn clear_telegram_account_profile(&self) {
        {
            let mut guard = self.telegram_account.write().await;
            *guard = None;
        }
        crate::storage::app_db::clear_telegram_account(&self.data_dir);
    }

    pub async fn effective_tier(&self) -> AccountTier {
        if !self.dynamic_limits_enabled {
            return AccountTier::Free;
        }

        let profile = self.telegram_account.read().await.clone();
        match profile {
            Some(profile) => {
                if self.profile_is_fresh(&profile) || !self.fallback_to_free_on_error {
                    if profile.is_premium {
                        AccountTier::Premium
                    } else {
                        AccountTier::Free
                    }
                } else {
                    AccountTier::Free
                }
            }
            None => AccountTier::Free,
        }
    }

    pub async fn dynamic_fallback_mode(&self) -> bool {
        if !self.dynamic_limits_enabled || !self.fallback_to_free_on_error {
            return false;
        }

        match self.telegram_account.read().await.clone() {
            Some(profile) => !self.profile_is_fresh(&profile),
            None => true,
        }
    }

    pub async fn effective_max_file_size_bytes(&self) -> u64 {
        if !self.dynamic_limits_enabled {
            return self.max_file_size_bytes;
        }

        match self.effective_tier().await {
            AccountTier::Premium => self.premium_max_file_size_bytes,
            AccountTier::Free => self.max_file_size_bytes,
        }
    }

    pub async fn effective_daily_bandwidth_limit_bytes(&self) -> u64 {
        if !self.dynamic_limits_enabled {
            return self.free_daily_bandwidth_limit_bytes;
        }

        match self.effective_tier().await {
            AccountTier::Premium => self.premium_daily_bandwidth_limit_bytes,
            AccountTier::Free => self.free_daily_bandwidth_limit_bytes,
        }
    }
}

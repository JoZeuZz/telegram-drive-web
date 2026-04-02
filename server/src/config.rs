#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AppEnv {
    Development,
    Production,
}

impl AppEnv {
    fn from_raw(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "production" | "prod" => Self::Production,
            _ => Self::Development,
        }
    }

    pub fn is_production(self) -> bool {
        matches!(self, Self::Production)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

/// Server configuration loaded from environment variables.
#[derive(Clone)]
pub struct Config {
    pub app_env: AppEnv,
    pub host: String,
    pub port: u16,
    pub frontend_port: u16,
    pub cors_allowed_origin: String,
    pub data_dir: String,
    pub cache_dir: String,
    pub session_secret: String,
    pub cookie_secure: bool,
    pub session_ttl_hours: i64,
    pub max_file_size_bytes: u64,
    pub premium_max_file_size_bytes: u64,
    pub free_daily_bandwidth_limit_bytes: u64,
    pub premium_daily_bandwidth_limit_bytes: u64,
    pub dynamic_limits_enabled: bool,
    pub forums_enabled: bool,
    pub fallback_to_free_on_error: bool,
    pub premium_detection_ttl_secs: u64,
    pub admin_password: String,
    pub trust_proxy_headers: bool,
    pub app_auth_rate_limit_max_requests: u32,
    pub app_auth_rate_limit_window_secs: u64,
    pub telegram_auth_rate_limit_max_requests: u32,
    pub telegram_auth_rate_limit_window_secs: u64,
    pub telegram_api_id: i32,
    pub telegram_api_hash: String,
}

fn env_bool(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|raw| {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn env_positive_i64(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn env_positive_u32(name: &str, default: u32) -> u32 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn env_positive_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

fn is_weak_session_secret(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 32 {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    let weak_markers = ["changeme", "change_me", "example", "replace-with"];
    weak_markers.iter().any(|marker| lower.contains(marker))
}

fn is_weak_admin_password(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.len() < 12 {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    matches!(lower.as_str(), "changeme" | "password" | "admin")
}

fn is_insecure_production_origin(value: &str) -> bool {
    let trimmed = value.trim();
    if !trimmed.starts_with("https://") {
        return true;
    }
    let lower = trimmed.to_ascii_lowercase();
    lower.contains("localhost") || lower.contains("127.0.0.1") || lower.contains("[::1]")
}

impl Config {
    pub fn from_env() -> Self {
        let app_env = AppEnv::from_raw(
            &std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
        );
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let cache_dir =
            std::env::var("CACHE_DIR").unwrap_or_else(|_| format!("{}/cache", data_dir));
        let frontend_port = std::env::var("FRONTEND_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let cors_allowed_origin = std::env::var("CORS_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| format!("http://localhost:{}", frontend_port));
        let session_secret = match env_non_empty("SESSION_SECRET") {
            Some(value) => value,
            None if app_env.is_production() => {
                tracing::error!("SESSION_SECRET is required in production and cannot be empty");
                String::new()
            }
            None => {
                tracing::warn!(
                    "SESSION_SECRET not set — using random ephemeral secret (sessions reset on restart)"
                );
                uuid::Uuid::new_v4().to_string()
            }
        };

        let free_max_file_size_bytes = env_positive_u64("MAX_FILE_SIZE_BYTES", 2_097_152_000);

        Self {
            app_env,
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            frontend_port,
            cors_allowed_origin,
            data_dir,
            cache_dir,
            session_secret,
            cookie_secure: env_bool("COOKIE_SECURE", false),
            session_ttl_hours: env_positive_i64("SESSION_TTL_HOURS", 8),
            max_file_size_bytes: free_max_file_size_bytes,
            premium_max_file_size_bytes: env_positive_u64(
                "PREMIUM_MAX_FILE_SIZE_BYTES",
                4_294_967_296,
            ),
            free_daily_bandwidth_limit_bytes: env_positive_u64(
                "FREE_DAILY_BANDWIDTH_LIMIT_BYTES",
                250 * 1024 * 1024 * 1024,
            ),
            premium_daily_bandwidth_limit_bytes: env_positive_u64(
                "PREMIUM_DAILY_BANDWIDTH_LIMIT_BYTES",
                800 * 1024 * 1024 * 1024,
            ),
            dynamic_limits_enabled: env_bool("DYNAMIC_LIMITS_ENABLED", true),
            forums_enabled: env_bool("FORUMS_ENABLED", true),
            fallback_to_free_on_error: env_bool("FALLBACK_TO_FREE_ON_ERROR", true),
            premium_detection_ttl_secs: env_positive_u64("PREMIUM_DETECTION_TTL_SECS", 3600),
            admin_password: std::env::var("ADMIN_PASSWORD")
                .unwrap_or_else(|_| "changeme".to_string()),
            trust_proxy_headers: env_bool("TRUST_PROXY_HEADERS", false),
            app_auth_rate_limit_max_requests: env_positive_u32(
                "APP_AUTH_RATE_LIMIT_MAX_REQUESTS",
                10,
            ),
            app_auth_rate_limit_window_secs: env_positive_u64(
                "APP_AUTH_RATE_LIMIT_WINDOW_SECS",
                60,
            ),
            telegram_auth_rate_limit_max_requests: env_positive_u32(
                "TELEGRAM_AUTH_RATE_LIMIT_MAX_REQUESTS",
                5,
            ),
            telegram_auth_rate_limit_window_secs: env_positive_u64(
                "TELEGRAM_AUTH_RATE_LIMIT_WINDOW_SECS",
                60,
            ),
            telegram_api_id: std::env::var("TELEGRAM_API_ID")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            telegram_api_hash: std::env::var("TELEGRAM_API_HASH").unwrap_or_default(),
        }
    }

    /// Validate runtime security policy.
    ///
    /// In production this is strict and fails startup on unsafe values.
    /// In development this only emits warnings to keep local DX simple.
    pub fn validate_runtime_security(&self) -> Result<(), String> {
        if self.app_env.is_production() {
            let mut issues = Vec::new();
            if !self.cookie_secure {
                issues.push("COOKIE_SECURE must be true in production");
            }
            if is_weak_session_secret(&self.session_secret) {
                issues.push("SESSION_SECRET is missing, weak, or placeholder-like");
            }
            if is_weak_admin_password(&self.admin_password) {
                issues.push("ADMIN_PASSWORD is weak or uses a default value");
            }
            if is_insecure_production_origin(&self.cors_allowed_origin) {
                issues.push("CORS_ALLOWED_ORIGIN must be an HTTPS public origin in production");
            }

            if !issues.is_empty() {
                let details = issues
                    .iter()
                    .map(|issue| format!("- {}", issue))
                    .collect::<Vec<_>>()
                    .join("\n");
                return Err(format!(
                    "Invalid production security configuration:\n{}",
                    details
                ));
            }
        } else {
            if !self.cookie_secure {
                tracing::warn!("Development mode: COOKIE_SECURE=false");
            }
            if is_weak_session_secret(&self.session_secret) {
                tracing::warn!("Development mode: SESSION_SECRET appears weak or placeholder-like");
            }
            if is_weak_admin_password(&self.admin_password) {
                tracing::warn!("Development mode: ADMIN_PASSWORD appears weak or default-like");
            }
            if is_insecure_production_origin(&self.cors_allowed_origin) {
                tracing::warn!(
                    "Development mode: CORS_ALLOWED_ORIGIN is not a production-grade HTTPS public origin"
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AppEnv, Config};

    fn strong_config() -> Config {
        Config {
            app_env: AppEnv::Production,
            host: "0.0.0.0".into(),
            port: 8080,
            frontend_port: 3000,
            cors_allowed_origin: "https://drive.example.com".into(),
            data_dir: "./data".into(),
            cache_dir: "./data/cache".into(),
            session_secret: "9a6adce2dc9681ed2f90a4c6f5c35f1f54d6f3478c4d27d8f9cbfa67e1ad6a92"
                .into(),
            cookie_secure: true,
            session_ttl_hours: 8,
            max_file_size_bytes: 512_u64 * 1024 * 1024,
            premium_max_file_size_bytes: 1024_u64 * 1024 * 1024,
            free_daily_bandwidth_limit_bytes: 250_u64 * 1024 * 1024 * 1024,
            premium_daily_bandwidth_limit_bytes: 800_u64 * 1024 * 1024 * 1024,
            dynamic_limits_enabled: true,
            forums_enabled: true,
            fallback_to_free_on_error: true,
            premium_detection_ttl_secs: 3600,
            admin_password: "correct-horse-battery-staple".into(),
            trust_proxy_headers: true,
            app_auth_rate_limit_max_requests: 10,
            app_auth_rate_limit_window_secs: 60,
            telegram_auth_rate_limit_max_requests: 5,
            telegram_auth_rate_limit_window_secs: 60,
            telegram_api_id: 123,
            telegram_api_hash: "hash".into(),
        }
    }

    #[test]
    fn production_validation_accepts_strong_values() {
        let cfg = strong_config();
        assert!(cfg.validate_runtime_security().is_ok());
    }

    #[test]
    fn production_validation_rejects_insecure_values() {
        let mut cfg = strong_config();
        cfg.cookie_secure = false;
        cfg.session_secret = "changeme".into();
        cfg.admin_password = "changeme".into();
        cfg.cors_allowed_origin = "http://localhost:3000".into();

        let err = cfg
            .validate_runtime_security()
            .expect_err("production config should be rejected");
        assert!(err.contains("COOKIE_SECURE"));
        assert!(err.contains("SESSION_SECRET"));
        assert!(err.contains("ADMIN_PASSWORD"));
        assert!(err.contains("CORS_ALLOWED_ORIGIN"));
    }

    #[test]
    fn development_allows_defaults() {
        let mut cfg = strong_config();
        cfg.app_env = AppEnv::Development;
        cfg.cookie_secure = false;
        cfg.session_secret = "changeme".into();
        cfg.admin_password = "changeme".into();
        cfg.cors_allowed_origin = "http://localhost:3000".into();
        assert!(cfg.validate_runtime_security().is_ok());
    }
}

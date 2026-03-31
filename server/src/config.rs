/// Server configuration loaded from environment variables.
#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub frontend_port: u16,
    pub cors_allowed_origin: String,
    pub data_dir: String,
    pub cache_dir: String,
    pub session_secret: String,
    pub cookie_secure: bool,
    pub session_ttl_hours: i64,
    pub admin_password: String,
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

fn env_positive_i64(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(default)
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let cache_dir = std::env::var("CACHE_DIR")
            .unwrap_or_else(|_| format!("{}/cache", data_dir));
        let frontend_port = std::env::var("FRONTEND_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);
        let cors_allowed_origin = std::env::var("CORS_ALLOWED_ORIGIN")
            .unwrap_or_else(|_| format!("http://localhost:{}", frontend_port));

        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            frontend_port,
            cors_allowed_origin,
            data_dir,
            cache_dir,
            session_secret: std::env::var("SESSION_SECRET")
                .unwrap_or_else(|_| {
                    tracing::warn!("SESSION_SECRET not set — using random ephemeral secret");
                    uuid::Uuid::new_v4().to_string()
                }),
            cookie_secure: env_bool("COOKIE_SECURE", false),
            session_ttl_hours: env_positive_i64("SESSION_TTL_HOURS", 8),
            admin_password: std::env::var("ADMIN_PASSWORD")
                .unwrap_or_else(|_| "changeme".to_string()),
            telegram_api_id: std::env::var("TELEGRAM_API_ID")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            telegram_api_hash: std::env::var("TELEGRAM_API_HASH")
                .unwrap_or_default(),
        }
    }
}

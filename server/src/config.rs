/// Server configuration loaded from environment variables.
#[derive(Clone)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub frontend_port: u16,
    pub data_dir: String,
    pub cache_dir: String,
    pub session_secret: String,
    pub admin_password: String,
    pub telegram_api_id: i32,
    pub telegram_api_hash: String,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir = std::env::var("DATA_DIR").unwrap_or_else(|_| "./data".to_string());
        let cache_dir = std::env::var("CACHE_DIR")
            .unwrap_or_else(|_| format!("{}/cache", data_dir));

        Self {
            host: std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            port: std::env::var("PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(8080),
            frontend_port: std::env::var("FRONTEND_PORT")
                .ok()
                .and_then(|p| p.parse().ok())
                .unwrap_or(3000),
            data_dir,
            cache_dir,
            session_secret: std::env::var("SESSION_SECRET")
                .unwrap_or_else(|_| {
                    tracing::warn!("SESSION_SECRET not set — using random ephemeral secret");
                    uuid::Uuid::new_v4().to_string()
                }),
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

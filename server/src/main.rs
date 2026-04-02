use actix_session::{config::PersistentSession, storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::Key;
use std::sync::Arc;

use telegram_drive_server::{
    app_state::AppState,
    config::Config,
    http, jobs,
    services::{
        bandwidth::BandwidthManager, bootstrap, upload_progress::UploadProgressManager,
        upload_queue::UploadQueue,
    },
};

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load .env before anything else so RUST_LOG takes effect
    dotenvy::dotenv().ok();

    // Initialize logging — supports LOG_FORMAT=json for structured output
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    if std::env::var("LOG_FORMAT").as_deref() == Ok("json") {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(true)
            .init();
    } else {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    }

    // Load configuration
    let config = Config::from_env();
    config
        .validate_runtime_security()
        .map_err(|msg| std::io::Error::new(std::io::ErrorKind::InvalidInput, msg))?;

    if !config.cookie_secure && !config.app_env.is_production() {
        tracing::warn!(
            "COOKIE_SECURE=false: session cookie will be sent over plain HTTP. Use true behind TLS reverse proxy."
        );
    }

    tracing::info!(
        "Starting telegram-drive-server v{}",
        env!("CARGO_PKG_VERSION")
    );
    tracing::info!("Listening on {}:{}", config.host, config.port);
    tracing::info!("Runtime environment: {}", config.app_env.as_str());
    tracing::info!(
        "Rate limiting: app-auth={}/{}s telegram-auth={}/{}s trust_proxy_headers={}",
        config.app_auth_rate_limit_max_requests,
        config.app_auth_rate_limit_window_secs,
        config.telegram_auth_rate_limit_max_requests,
        config.telegram_auth_rate_limit_window_secs,
        config.trust_proxy_headers
    );

    // Ensure data directory exists before bootstrap
    std::fs::create_dir_all(&config.data_dir).expect("Failed to create data directory");

    // Bootstrap admin user (hash password, persist to admin.json)
    let admin_hash = bootstrap::ensure_admin(&config.data_dir, &config.admin_password)
        .expect("Failed to bootstrap admin user");

    // Build cookie signing key from session secret (pad/hash to 64 bytes)
    let key_bytes = derive_cookie_key(&config.session_secret);
    let cookie_key = Key::from(&key_bytes);

    // Initialize shared state
    let state_arc = Arc::new(AppState::new(&config, admin_hash));
    let bw_arc = Arc::new(BandwidthManager::with_limits(
        &config.data_dir,
        config.free_daily_bandwidth_limit_bytes,
        config.premium_daily_bandwidth_limit_bytes,
    ));
    let upload_queue = UploadQueue::new(state_arc.clone(), bw_arc.clone(), 3);
    let upload_progress = actix_web::web::Data::new(UploadProgressManager::new());

    let state = actix_web::web::Data::from(state_arc.clone());
    let bw = actix_web::web::Data::from(bw_arc);
    let upload_queue = actix_web::web::Data::new(upload_queue);

    // Spawn background jobs
    jobs::cleanup::spawn(state_arc.clone());
    jobs::reconnect::spawn(state_arc);

    // Extract bind address before moving config into closure
    let bind_host = config.host.clone();
    let bind_port = config.port;
    let route_config = http::RouteConfig::from_config(&config);
    let max_payload_bytes = if config.dynamic_limits_enabled {
        config
            .max_file_size_bytes
            .max(config.premium_max_file_size_bytes)
    } else {
        config.max_file_size_bytes
    };
    let payload_limit = usize::try_from(max_payload_bytes).unwrap_or(usize::MAX);

    // Start HTTP server
    actix_web::HttpServer::new(move || {
        let cors = actix_cors::Cors::default()
            .allowed_origin(&config.cors_allowed_origin)
            .allow_any_method()
            .allow_any_header()
            .supports_credentials();

        // Cookie-based session middleware
        let session = SessionMiddleware::builder(CookieSessionStore::default(), cookie_key.clone())
            .cookie_http_only(true)
            .cookie_same_site(actix_web::cookie::SameSite::Strict)
            .cookie_secure(config.cookie_secure)
            .session_lifecycle(PersistentSession::default().session_ttl(
                actix_web::cookie::time::Duration::hours(config.session_ttl_hours),
            ))
            .cookie_name("td_session".to_string())
            .build();

        actix_web::App::new()
            .wrap(cors)
            .wrap(session)
            .wrap(http::middleware::request_id::RequestId)
            .wrap(http::middleware::logging::create_logger())
            .app_data(state.clone())
            .app_data(bw.clone())
            .app_data(upload_queue.clone())
            .app_data(upload_progress.clone())
            .app_data(actix_web::web::PayloadConfig::new(payload_limit))
            .configure(|cfg| http::configure_routes(cfg, route_config))
    })
    .bind((bind_host.as_str(), bind_port))?
    .run()
    .await
}

/// Derive a 64-byte key from the session secret using SHA-512.
fn derive_cookie_key(secret: &str) -> [u8; 64] {
    use sha2::{Digest, Sha512};

    let digest = Sha512::digest(secret.as_bytes());
    let mut buf = [0u8; 64];
    buf.copy_from_slice(&digest);
    buf
}

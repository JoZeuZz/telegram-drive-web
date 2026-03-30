use actix_session::{storage::CookieSessionStore, SessionMiddleware};
use actix_web::cookie::Key;
use std::sync::Arc;

use telegram_drive_server::{
    app_state::AppState,
    config::Config,
    http,
    jobs,
    services::{bandwidth::BandwidthManager, bootstrap, upload_queue::UploadQueue},
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
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .init();
    }

    // Load configuration
    let config = Config::from_env();

    tracing::info!("Starting telegram-drive-server v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!("Listening on {}:{}", config.host, config.port);

    // Ensure data directory exists before bootstrap
    std::fs::create_dir_all(&config.data_dir)
        .expect("Failed to create data directory");

    // Bootstrap admin user (hash password, persist to admin.json)
    let admin_hash = bootstrap::ensure_admin(&config.data_dir, &config.admin_password)
        .expect("Failed to bootstrap admin user");

    // Build cookie signing key from session secret (pad/hash to 64 bytes)
    let key_bytes = derive_cookie_key(&config.session_secret);
    let cookie_key = Key::from(&key_bytes);

    // Initialize shared state
    let state_arc = Arc::new(AppState::new(&config, admin_hash));
    let bw_arc = Arc::new(BandwidthManager::new(&config.data_dir));
    let upload_queue = UploadQueue::new(state_arc.clone(), bw_arc.clone(), 3);

    let state = actix_web::web::Data::from(state_arc.clone());
    let bw = actix_web::web::Data::from(bw_arc);
    let upload_queue = actix_web::web::Data::new(upload_queue);

    // Spawn background jobs
    jobs::cleanup::spawn(state_arc.clone());
    jobs::reconnect::spawn(state_arc);

    // Extract bind address before moving config into closure
    let bind_host = config.host.clone();
    let bind_port = config.port;

    // Start HTTP server
    actix_web::HttpServer::new(move || {
        let cors = actix_cors::Cors::default()
            .allowed_origin(&format!("http://localhost:{}", config.frontend_port))
            .allow_any_method()
            .allow_any_header()
            .supports_credentials();

        // Cookie-based session middleware
        let session = SessionMiddleware::builder(
            CookieSessionStore::default(),
            cookie_key.clone(),
        )
        .cookie_http_only(true)
        .cookie_same_site(actix_web::cookie::SameSite::Lax)
        .cookie_secure(false) // set true behind HTTPS reverse proxy
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
            .app_data(actix_web::web::PayloadConfig::new(512 * 1024 * 1024))
            .configure(http::configure_routes)
    })
    .bind((bind_host.as_str(), bind_port))?
    .run()
    .await
}

/// Derive a 64-byte key from the session secret using SHA-512.
fn derive_cookie_key(secret: &str) -> [u8; 64] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut buf = [0u8; 64];
    let secret_bytes = secret.as_bytes();

    // Fill the buffer by repeating the secret and mixing with position
    for (i, byte) in buf.iter_mut().enumerate() {
        let mut hasher = DefaultHasher::new();
        i.hash(&mut hasher);
        secret_bytes.hash(&mut hasher);
        let h = hasher.finish();
        *byte = h.to_le_bytes()[i % 8];
    }
    buf
}

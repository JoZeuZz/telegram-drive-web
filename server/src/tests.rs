use actix_session::config::PersistentSession;
use actix_session::storage::CookieSessionStore;
use actix_session::SessionMiddleware;
use actix_web::cookie::{Key, SameSite};
use actix_web::http::StatusCode;
use actix_web::{test, web, App};
use std::sync::Arc;

use crate::app_state::AppState;
use crate::config::{AppEnv, Config};
use crate::domain::dto::UploadQuery;
use crate::http;
use crate::services::bandwidth::BandwidthManager;
use crate::services::bootstrap;
use crate::services::upload_progress::UploadProgressManager;
use crate::services::upload_queue::UploadQueue;

fn test_config(data_dir: &str) -> Config {
    Config {
        host: "127.0.0.1".into(),
        port: 0,
        app_env: AppEnv::Development,
        frontend_port: 3000,
        cors_allowed_origin: "http://localhost:3000".into(),
        data_dir: data_dir.into(),
        cache_dir: format!("{}/cache", data_dir),
        session_secret: "test-secret-for-integration-tests".into(),
        cookie_secure: false,
        session_ttl_hours: 8,
        max_file_size_bytes: 2_097_152_000,
        admin_password: "testpass".into(),
        trust_proxy_headers: false,
        app_auth_rate_limit_max_requests: 10,
        app_auth_rate_limit_window_secs: 60,
        telegram_auth_rate_limit_max_requests: 5,
        telegram_auth_rate_limit_window_secs: 60,
        telegram_api_id: 0,
        telegram_api_hash: String::new(),
    }
}

struct TestEnv {
    state: web::Data<AppState>,
    bw: web::Data<BandwidthManager>,
    queue: web::Data<UploadQueue>,
    progress: web::Data<UploadProgressManager>,
    route_config: http::RouteConfig,
    cookie_key: Key,
}

impl TestEnv {
    fn new(dir: &str) -> Self {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        let config = test_config(dir);
        let hash = bootstrap::hash_password("testpass").unwrap();
        let state_arc = Arc::new(AppState::new(&config, hash));
        let bw_arc = Arc::new(BandwidthManager::new(dir));
        let queue = UploadQueue::new(state_arc.clone(), bw_arc.clone(), 2);
        let progress = UploadProgressManager::new();

        Self {
            state: web::Data::from(state_arc),
            bw: web::Data::from(bw_arc),
            queue: web::Data::new(queue),
            progress: web::Data::new(progress),
            route_config: http::RouteConfig::from_config(&config),
            cookie_key: Key::generate(),
        }
    }
}

macro_rules! test_app {
    ($env:expr) => {{
        let session =
            SessionMiddleware::builder(CookieSessionStore::default(), $env.cookie_key.clone())
                .cookie_http_only(true)
                .cookie_same_site(SameSite::Strict)
                .cookie_secure(false)
                .session_lifecycle(
                    PersistentSession::default()
                        .session_ttl(actix_web::cookie::time::Duration::hours(8)),
                )
                .cookie_name("td_session".into())
                .build();

        test::init_service(
            App::new()
                .wrap(session)
                .app_data($env.state.clone())
                .app_data($env.bw.clone())
                .app_data($env.queue.clone())
                .app_data($env.progress.clone())
                .app_data(web::PayloadConfig::new(
                    usize::try_from($env.state.max_file_size_bytes).unwrap_or(usize::MAX),
                ))
                .configure(|cfg| http::configure_routes(cfg, $env.route_config)),
        )
        .await
    }};
}

/// Helper macro: login and return the session cookie string.
macro_rules! login_get_cookie {
    ($app:expr) => {{
        let req = test::TestRequest::post()
            .uri("/api/app-auth/login")
            .insert_header(("X-Requested-With", "XMLHttpRequest"))
            .set_json(serde_json::json!({"password": "testpass"}))
            .to_request();
        let resp = test::call_service(&$app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);

        let cookie_header = resp
            .response()
            .headers()
            .get(actix_web::http::header::SET_COOKIE)
            .expect("Set-Cookie header missing")
            .to_str()
            .unwrap()
            .to_string();

        cookie_header
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }};
}

// ─── Health & Version ────────────────────────────────────

#[actix_web::test]
async fn health_returns_ok() {
    let env = TestEnv::new("/tmp/td_test_health");
    let app = test_app!(env);

    let req = test::TestRequest::get().uri("/api/health").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["status"], "ok");
}

#[actix_web::test]
async fn version_returns_ok() {
    let env = TestEnv::new("/tmp/td_test_version");
    let app = test_app!(env);

    let req = test::TestRequest::get().uri("/api/version").to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["name"], "telegram-drive-server");
}

// ─── Auth Protection ─────────────────────────────────────

#[actix_web::test]
async fn protected_routes_require_auth() {
    let env = TestEnv::new("/tmp/td_test_protected");
    let app = test_app!(env);

    let routes = vec![
        "/api/files",
        "/api/folders",
        "/api/search?q=test",
        "/api/bandwidth",
        "/api/uploads",
        "/api/telegram/auth/status",
    ];

    for uri in routes {
        let req = test::TestRequest::get().uri(uri).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::UNAUTHORIZED,
            "Expected 401 for {}",
            uri
        );
    }
}

// ─── Login Flow ──────────────────────────────────────────

#[actix_web::test]
async fn login_wrong_password_returns_401() {
    let env = TestEnv::new("/tmp/td_test_login_bad");
    let app = test_app!(env);

    let req = test::TestRequest::post()
        .uri("/api/app-auth/login")
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .set_json(serde_json::json!({"password": "wrong"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[actix_web::test]
async fn login_correct_password_sets_cookie() {
    let env = TestEnv::new("/tmp/td_test_login_ok");
    let app = test_app!(env);

    let req = test::TestRequest::post()
        .uri("/api/app-auth/login")
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .set_json(serde_json::json!({"password": "testpass"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Session cookie should be set
    let has_cookie = resp.response().cookies().any(|c| c.name() == "td_session");
    assert!(has_cookie, "Session cookie td_session not set");

    let set_cookie = resp
        .response()
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .expect("Set-Cookie header missing")
        .to_str()
        .unwrap()
        .to_string();

    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("SameSite=Strict"));
}

#[actix_web::test]
async fn authenticated_user_can_access_protected_routes() {
    let env = TestEnv::new("/tmp/td_test_auth_access");
    let app = test_app!(env);

    let cookie_str = login_get_cookie!(app);

    // Access protected route (bandwidth — doesn't need Telegram)
    let req = test::TestRequest::get()
        .uri("/api/bandwidth")
        .insert_header(("Cookie", cookie_str.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["up_bytes"].is_number());
    assert!(body["down_bytes"].is_number());
}

// ─── Upload Queue ────────────────────────────────────────

#[actix_web::test]
async fn upload_queue_starts_empty() {
    let env = TestEnv::new("/tmp/td_test_queue_empty");
    let app = test_app!(env);

    let cookie_str = login_get_cookie!(app);

    // Check upload queue
    let req = test::TestRequest::get()
        .uri("/api/uploads")
        .insert_header(("Cookie", cookie_str.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body.as_array().unwrap().is_empty());
}

// ─── Auth Status ─────────────────────────────────────────

#[actix_web::test]
async fn auth_status_reflects_login_state() {
    let env = TestEnv::new("/tmp/td_test_auth_status");
    let app = test_app!(env);

    // Unauthenticated
    let req = test::TestRequest::get()
        .uri("/api/app-auth/status")
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["authenticated"], false);

    // Login
    let cookie_str = login_get_cookie!(app);

    // Authenticated
    let req = test::TestRequest::get()
        .uri("/api/app-auth/status")
        .insert_header(("Cookie", cookie_str.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["authenticated"], true);
}

// ─── Logout ──────────────────────────────────────────────

#[actix_web::test]
async fn logout_clears_session() {
    let env = TestEnv::new("/tmp/td_test_logout");
    let app = test_app!(env);

    // Login
    let cookie_str = login_get_cookie!(app);

    // Logout — get the new (cleared) cookie from response
    let req = test::TestRequest::post()
        .uri("/api/app-auth/logout")
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .insert_header(("Cookie", cookie_str.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Extract the replacement cookie set by the logout response
    let new_cookie = resp
        .response()
        .headers()
        .get(actix_web::http::header::SET_COOKIE)
        .map(|v| v.to_str().unwrap().split(';').next().unwrap().to_string())
        .unwrap_or_default();

    // After logout, the new cookie should not be authenticated
    let req = test::TestRequest::get()
        .uri("/api/app-auth/status")
        .insert_header(("Cookie", new_cookie.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    let body: serde_json::Value = test::read_body_json(resp).await;
    assert_eq!(body["authenticated"], false);
}

// ─── Metrics ─────────────────────────────────────────────

#[actix_web::test]
async fn metrics_returns_operational_data() {
    let env = TestEnv::new("/tmp/td_test_metrics");
    let app = test_app!(env);

    let cookie_str = login_get_cookie!(app);

    let req = test::TestRequest::get()
        .uri("/api/metrics")
        .insert_header(("Cookie", cookie_str.as_str()))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["uptime_secs"].is_number());
    assert!(body["cache_bytes"].is_number());
    assert!(body["cache_files"].is_number());
    assert_eq!(body["max_file_size_bytes"], 2_097_152_000u64);
    assert!(body["bandwidth"]["date"].is_string());
    assert_eq!(body["telegram_connected"], false);
    assert_eq!(body["upload_queue_length"], 0);
}

// ─── Admin Cache Clean ───────────────────────────────────

#[actix_web::test]
async fn admin_clean_cache_returns_counts() {
    let env = TestEnv::new("/tmp/td_test_admin_cache");
    let app = test_app!(env);

    let cookie_str = login_get_cookie!(app);

    let req = test::TestRequest::post()
        .uri("/api/admin/clean-cache")
        .insert_header(("Cookie", cookie_str.as_str()))
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body: serde_json::Value = test::read_body_json(resp).await;
    assert!(body["files_removed"].is_number());
    assert!(body["bytes_freed"].is_number());
}

// ─── Password Change (Bootstrap) ─────────────────────────

#[actix_web::test]
async fn bootstrap_changes_password() {
    let env = TestEnv::new("/tmp/td_test_bootstrap");
    let app = test_app!(env);

    let cookie_str = login_get_cookie!(app);

    // Change password
    let req = test::TestRequest::post()
        .uri("/api/app-auth/bootstrap")
        .insert_header(("Cookie", cookie_str.as_str()))
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .set_json(serde_json::json!({
            "current_password": "testpass",
            "new_password": "newpass123"
        }))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    // Old password should fail
    let req = test::TestRequest::post()
        .uri("/api/app-auth/login")
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .set_json(serde_json::json!({"password": "testpass"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // New password should work
    let req = test::TestRequest::post()
        .uri("/api/app-auth/login")
        .insert_header(("X-Requested-With", "XMLHttpRequest"))
        .set_json(serde_json::json!({"password": "newpass123"}))
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[actix_web::test]
async fn upload_query_defaults_to_document_mode() {
    let query = web::Query::<UploadQuery>::from_query("folder_id=42")
        .expect("query should parse with defaults");
    assert_eq!(query.folder_id, Some(42));
    assert!(!query.queue);
    assert!(!query.as_photo);
    assert_eq!(query.upload_id, None);
    assert_eq!(query.upload_size_bytes, None);
}

#[actix_web::test]
async fn upload_query_parses_photo_mode_flag() {
    let query = web::Query::<UploadQuery>::from_query(
        "queue=true&as_photo=true&upload_id=test-123&upload_size_bytes=2048",
    )
        .expect("query should parse explicit flags");
    assert!(query.queue);
    assert!(query.as_photo);
    assert_eq!(query.upload_id.as_deref(), Some("test-123"));
    assert_eq!(query.upload_size_bytes, Some(2048));
}

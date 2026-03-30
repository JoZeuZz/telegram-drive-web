use actix_session::Session;
use actix_web::{get, post, web, HttpResponse};

use crate::app_state::AppState;
use crate::domain::dto::{AuthStatusResponse, LoginRequest, LoginResponse};
use crate::errors::AppError;
use crate::http::middleware::auth::{clear_session, mark_authenticated};
use crate::services::bootstrap::{hash_password, verify_password};
use crate::storage;

/// POST /api/app-auth/login
#[post("/login")]
async fn login(
    body: web::Json<LoginRequest>,
    state: web::Data<AppState>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    let hash = state.admin_password_hash.read().await;
    if verify_password(&body.password, &hash) {
        mark_authenticated(&session);
        Ok(HttpResponse::Ok().json(LoginResponse { success: true }))
    } else {
        Err(AppError::Unauthorized)
    }
}

/// POST /api/app-auth/logout
#[post("/logout")]
async fn logout(session: Session) -> HttpResponse {
    clear_session(&session);
    HttpResponse::Ok().json(LoginResponse { success: true })
}

/// GET /api/app-auth/status
#[get("/status")]
async fn status(session: Session) -> HttpResponse {
    let authenticated = session
        .get::<bool>("authenticated")
        .unwrap_or(None)
        .unwrap_or(false);
    HttpResponse::Ok().json(AuthStatusResponse { authenticated })
}

/// POST /api/app-auth/bootstrap — change admin password.
#[post("/bootstrap")]
async fn bootstrap(
    body: web::Json<BootstrapRequest>,
    state: web::Data<AppState>,
    session: Session,
) -> Result<HttpResponse, AppError> {
    // Must be authenticated
    let authenticated = session
        .get::<bool>("authenticated")
        .unwrap_or(None)
        .unwrap_or(false);
    if !authenticated {
        return Err(AppError::Unauthorized);
    }

    // Verify current password
    let current_hash = state.admin_password_hash.read().await;
    if !verify_password(&body.current_password, &current_hash) {
        return Err(AppError::BadRequest("Current password is incorrect".into()));
    }
    drop(current_hash);

    // Hash new password and persist
    let new_hash = hash_password(&body.new_password)?;
    storage::app_db::save_admin_hash(&state.data_dir, &new_hash)?;

    // Update in-memory hash
    let mut hash = state.admin_password_hash.write().await;
    *hash = new_hash;

    Ok(HttpResponse::Ok().json(LoginResponse { success: true }))
}

#[derive(serde::Deserialize)]
struct BootstrapRequest {
    current_password: String,
    new_password: String,
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(login)
        .service(logout)
        .service(status)
        .service(bootstrap);
}

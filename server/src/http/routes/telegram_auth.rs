use actix_web::{get, post, web, HttpResponse};

use crate::app_state::AppState;
use crate::domain::dto::{
    MessageResponse, TelegramCheckPasswordRequest, TelegramConnectRequest,
    TelegramRequestCodeRequest, TelegramSignInRequest, TelegramStatusResponse,
};
use crate::errors::AppError;
use crate::services::telegram_auth;

/// POST /api/telegram/auth/connect
#[post("/connect")]
async fn connect(
    body: web::Json<TelegramConnectRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    telegram_auth::connect(&state, body.api_id).await?;
    Ok(HttpResponse::Ok().json(MessageResponse {
        message: "Connected".to_string(),
    }))
}

/// GET /api/telegram/auth/status
#[get("/status")]
async fn status(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    let connected = telegram_auth::check_connection(&state)
        .await
        .unwrap_or(false);
    Ok(HttpResponse::Ok().json(TelegramStatusResponse { connected }))
}

/// POST /api/telegram/auth/request-code
#[post("/request-code")]
async fn request_code(
    body: web::Json<TelegramRequestCodeRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let result =
        telegram_auth::request_code(&state, &body.phone, body.api_id, &body.api_hash).await?;
    Ok(HttpResponse::Ok().json(MessageResponse { message: result }))
}

/// POST /api/telegram/auth/sign-in
#[post("/sign-in")]
async fn sign_in(
    body: web::Json<TelegramSignInRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let result = telegram_auth::sign_in(&state, &body.code).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// POST /api/telegram/auth/check-password
#[post("/check-password")]
async fn check_password(
    body: web::Json<TelegramCheckPasswordRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    let result = telegram_auth::check_password(&state, &body.password).await?;
    Ok(HttpResponse::Ok().json(result))
}

/// POST /api/telegram/auth/logout
#[post("/logout")]
async fn logout(state: web::Data<AppState>) -> Result<HttpResponse, AppError> {
    telegram_auth::logout(&state).await?;
    Ok(HttpResponse::Ok().json(MessageResponse {
        message: "Logged out from Telegram".to_string(),
    }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(connect)
        .service(status)
        .service(request_code)
        .service(sign_in)
        .service(check_password)
        .service(logout);
}

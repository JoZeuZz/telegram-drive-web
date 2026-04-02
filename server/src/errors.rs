use actix_web::{HttpResponse, ResponseError};
use serde::Serialize;
use thiserror::Error;

/// Unified error type for the application.
#[derive(Debug, Error)]
pub enum AppError {
    #[error("Not authenticated")]
    Unauthorized,

    #[error("Forbidden")]
    Forbidden,

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Telegram error: {0}")]
    Telegram(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

impl ResponseError for AppError {
    fn error_response(&self) -> HttpResponse {
        let body = ErrorResponse {
            error: self.to_string(),
        };
        match self {
            AppError::Unauthorized => HttpResponse::Unauthorized().json(body),
            AppError::Forbidden => HttpResponse::Forbidden().json(body),
            AppError::NotFound(_) => HttpResponse::NotFound().json(body),
            AppError::BadRequest(_) => HttpResponse::BadRequest().json(body),
            AppError::Telegram(_) => HttpResponse::BadGateway().json(body),
            AppError::Internal(_) => HttpResponse::InternalServerError().json(body),
        }
    }
}

/// Map a FLOOD_WAIT error to a user-friendly message.
pub fn map_telegram_error(e: impl std::fmt::Display) -> AppError {
    let err_str = e.to_string();
    if err_str.contains("FLOOD_WAIT") {
        if let Some(start) = err_str.find("(value: ") {
            let rest = &err_str[start + 8..];
            if let Some(end) = rest.find(')') {
                if let Ok(seconds) = rest[..end].parse::<i64>() {
                    return AppError::Telegram(format!(
                        "Rate limited by Telegram. Retry after {} seconds.",
                        seconds
                    ));
                }
            }
        }
        return AppError::Telegram("Rate limited by Telegram. Retry later.".to_string());
    }
    AppError::Telegram(err_str)
}

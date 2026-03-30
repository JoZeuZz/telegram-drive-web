use actix_web::{get, web, HttpResponse};

use crate::app_state::AppState;
use crate::domain::dto::SearchQuery;
use crate::errors::AppError;
use crate::services::telegram_files;

/// GET /api/search?q=
#[get("")]
async fn search(
    query: web::Query<SearchQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if query.q.trim().is_empty() {
        return Err(AppError::BadRequest(
            "Query parameter 'q' is required".into(),
        ));
    }
    let results = telegram_files::search_global(&state, &query.q).await?;
    Ok(HttpResponse::Ok().json(results))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(search);
}

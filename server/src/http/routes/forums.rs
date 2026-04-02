use actix_web::{delete, get, post, web, HttpRequest, HttpResponse};

use crate::{
    app_state::AppState,
    domain::dto::{
        CreateForumRequest, CreateForumTopicRequest, DeleteForumTopicQuery, ListForumTopicsResponse,
        ListForumsResponse, SuccessResponse,
    },
    errors::AppError,
    services::telegram_forums,
};

#[cfg(test)]
fn maybe_stubbed_error(req: &HttpRequest) -> Option<AppError> {
    let raw = req
        .headers()
        .get("x-test-forums-stub-error")?
        .to_str()
        .ok()?
        .trim()
        .to_ascii_lowercase();

    match raw.as_str() {
        "telegram" => Some(AppError::Telegram(
            "Stubbed Telegram route failure".to_string(),
        )),
        "internal" => Some(AppError::Internal(
            "Stubbed internal route failure".to_string(),
        )),
        _ => None,
    }
}

#[cfg(not(test))]
fn maybe_stubbed_error(_req: &HttpRequest) -> Option<AppError> {
    None
}

#[get("")]
async fn list_forums(
    req: HttpRequest,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if let Some(err) = maybe_stubbed_error(&req) {
        return Err(err);
    }

    let forums = telegram_forums::list_forums(&state).await?;
    Ok(HttpResponse::Ok().json(ListForumsResponse { forums }))
}

#[post("")]
async fn create_forum(
    req: HttpRequest,
    body: web::Json<CreateForumRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if let Some(err) = maybe_stubbed_error(&req) {
        return Err(err);
    }

    let forum = telegram_forums::create_forum(&state, &body.name).await?;
    Ok(HttpResponse::Created().json(forum))
}

#[delete("/{forum_id}")]
async fn delete_forum(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if let Some(err) = maybe_stubbed_error(&req) {
        return Err(err);
    }

    let forum_id = path.into_inner();
    telegram_forums::delete_forum(&state, forum_id).await?;

    Ok(HttpResponse::Ok().json(SuccessResponse { success: true }))
}

#[get("/{forum_id}/topics")]
async fn list_topics(
    req: HttpRequest,
    path: web::Path<i64>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if let Some(err) = maybe_stubbed_error(&req) {
        return Err(err);
    }

    let forum_id = path.into_inner();
    let topics = telegram_forums::list_topics(&state, forum_id).await?;
    Ok(HttpResponse::Ok().json(ListForumTopicsResponse { topics }))
}

#[post("/{forum_id}/topics")]
async fn create_topic(
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<CreateForumTopicRequest>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if let Some(err) = maybe_stubbed_error(&req) {
        return Err(err);
    }

    let forum_id = path.into_inner();
    let topic = telegram_forums::create_topic(
        &state,
        forum_id,
        &body.title,
        body.icon_color,
        body.icon_emoji_id,
    )
    .await?;

    Ok(HttpResponse::Created().json(topic))
}

#[delete("/{forum_id}/topics/{topic_id}")]
async fn delete_topic(
    req: HttpRequest,
    path: web::Path<(i64, i32)>,
    query: web::Query<DeleteForumTopicQuery>,
    state: web::Data<AppState>,
) -> Result<HttpResponse, AppError> {
    if let Some(err) = maybe_stubbed_error(&req) {
        return Err(err);
    }

    let (forum_id, topic_id) = path.into_inner();
    telegram_forums::delete_topic(&state, forum_id, topic_id, query.top_message).await?;

    Ok(HttpResponse::Ok().json(SuccessResponse { success: true }))
}

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg.service(list_forums)
        .service(create_forum)
        .service(delete_forum)
        .service(list_topics)
        .service(create_topic)
        .service(delete_topic);
}

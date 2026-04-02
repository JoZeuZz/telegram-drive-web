pub mod middleware;
pub mod routes;

use crate::config::Config;
use actix_web::web;
use middleware::rate_limit::RateLimit;

#[derive(Clone, Copy)]
pub struct RouteConfig {
    pub trust_proxy_headers: bool,
    pub app_auth_rate_limit_max_requests: u32,
    pub app_auth_rate_limit_window_secs: u64,
    pub telegram_auth_rate_limit_max_requests: u32,
    pub telegram_auth_rate_limit_window_secs: u64,
}

impl RouteConfig {
    pub fn from_config(config: &Config) -> Self {
        Self {
            trust_proxy_headers: config.trust_proxy_headers,
            app_auth_rate_limit_max_requests: config.app_auth_rate_limit_max_requests,
            app_auth_rate_limit_window_secs: config.app_auth_rate_limit_window_secs,
            telegram_auth_rate_limit_max_requests: config.telegram_auth_rate_limit_max_requests,
            telegram_auth_rate_limit_window_secs: config.telegram_auth_rate_limit_window_secs,
        }
    }
}

pub fn configure_routes(cfg: &mut web::ServiceConfig, route_config: RouteConfig) {
    cfg.service(
        web::scope("/api")
            // ── Public (no session required) ─────────────────
            .service(routes::health::health_check)
            .service(routes::health::version_info)
            // ── App auth (login/logout/status — rate-limited) ──
            .service(
                web::scope("/app-auth")
                    .wrap(
                        RateLimit::new(
                            route_config.app_auth_rate_limit_max_requests,
                            route_config.app_auth_rate_limit_window_secs,
                        )
                        .with_trust_proxy_headers(route_config.trust_proxy_headers),
                    )
                    .wrap(middleware::csrf::CsrfCheck)
                    .configure(routes::app_auth::configure),
            )
            // ── Protected routes (require session cookie) ────
            .service(
                web::scope("")
                    .wrap(middleware::auth::RequireAuth)
                    .wrap(middleware::csrf::CsrfCheck)
                    .wrap(middleware::audit::AuditLog::new(
                        route_config.trust_proxy_headers,
                    ))
                    .service(
                        web::scope("/telegram/auth")
                            .wrap(
                                RateLimit::new(
                                    route_config.telegram_auth_rate_limit_max_requests,
                                    route_config.telegram_auth_rate_limit_window_secs,
                                )
                                .with_trust_proxy_headers(route_config.trust_proxy_headers),
                            )
                            .configure(routes::telegram_auth::configure),
                    )
                    .service(web::scope("/files").configure(routes::files::configure))
                    .service(web::scope("/folders").configure(routes::folders::configure))
                    .service(web::scope("/forums").configure(routes::forums::configure))
                    .service(web::scope("/search").configure(routes::search::configure))
                    .service(web::scope("/media").configure(routes::media::configure))
                    .service(web::scope("/bandwidth").configure(routes::bandwidth::configure))
                    .service(web::scope("/account-info").configure(routes::account::configure))
                    .service(web::scope("/uploads").configure(routes::uploads::configure))
                    .service(web::scope("/admin").configure(routes::admin::configure))
                    .service(web::scope("/metrics").configure(routes::metrics::configure)),
            ),
    );
}

pub mod routes;
pub mod middleware;

use actix_web::web;
use middleware::rate_limit::RateLimit;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api")
            // ── Public (no session required) ─────────────────
            .service(routes::health::health_check)
            .service(routes::health::version_info)
            // ── App auth (login/logout/status — rate-limited) ──
            .service(
                web::scope("/app-auth")
                    .wrap(RateLimit::new(10, 60)) // 10 req / 60 s per IP
                    .wrap(middleware::csrf::CsrfCheck)
                    .configure(routes::app_auth::configure),
            )
            // ── Protected routes (require session cookie) ────
            .service(
                web::scope("")
                    .wrap(middleware::auth::RequireAuth)
                    .wrap(middleware::csrf::CsrfCheck)
                    .wrap(middleware::audit::AuditLog)
                    .service(
                        web::scope("/telegram/auth")
                            .wrap(RateLimit::new(5, 60)) // 5 req / 60 s per IP
                            .configure(routes::telegram_auth::configure),
                    )
                    .service(
                        web::scope("/files")
                            .configure(routes::files::configure),
                    )
                    .service(
                        web::scope("/folders")
                            .configure(routes::folders::configure),
                    )
                    .service(
                        web::scope("/search")
                            .configure(routes::search::configure),
                    )
                    .service(
                        web::scope("/media")
                            .configure(routes::media::configure),
                    )
                    .service(
                        web::scope("/bandwidth")
                            .configure(routes::bandwidth::configure),
                    )
                    .service(
                        web::scope("/uploads")
                            .configure(routes::uploads::configure),
                    )
                    .service(
                        web::scope("/admin")
                            .configure(routes::admin::configure),
                    )
                    .service(
                        web::scope("/metrics")
                            .configure(routes::metrics::configure),
                    ),
            ),
    );
}

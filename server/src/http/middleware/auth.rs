use actix_session::SessionExt;
use actix_web::body::BoxBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::{Error, HttpResponse};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;

const SESSION_KEY: &str = "authenticated";

/// Middleware that rejects requests without a valid session cookie.
/// Attach to scopes that require login.
pub struct RequireAuth;

impl<S> Transform<S, ServiceRequest> for RequireAuth
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = RequireAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequireAuthMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct RequireAuthMiddleware<S> {
    service: Rc<S>,
}

impl<S> Service<ServiceRequest> for RequireAuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let session = req.get_session();
        let is_authenticated = session
            .get::<bool>(SESSION_KEY)
            .unwrap_or(None)
            .unwrap_or(false);

        if !is_authenticated {
            let response = HttpResponse::Unauthorized()
                .json(serde_json::json!({"error": "Not authenticated"}));
            return Box::pin(async move { Ok(req.into_response(response)) });
        }

        let srv = Rc::clone(&self.service);
        Box::pin(async move { srv.call(req).await })
    }
}

/// Helper: set session as authenticated.
pub fn mark_authenticated(session: &actix_session::Session) {
    let _ = session.insert(SESSION_KEY, true);
}

/// Helper: clear session.
pub fn clear_session(session: &actix_session::Session) {
    session.purge();
}

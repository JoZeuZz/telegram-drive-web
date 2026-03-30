use actix_web::body::BoxBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::Method;
use actix_web::{Error, HttpResponse};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;

const HEADER_NAME: &str = "x-requested-with";
const EXPECTED_VALUE: &str = "XMLHttpRequest";

/// Middleware that rejects mutating requests (POST, PUT, DELETE, PATCH)
/// unless they carry an `X-Requested-With: XMLHttpRequest` header.
///
/// This provides defense-in-depth against CSRF attacks alongside SameSite cookies.
pub struct CsrfCheck;

impl<S> Transform<S, ServiceRequest> for CsrfCheck
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = CsrfCheckMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(CsrfCheckMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct CsrfCheckMiddleware<S> {
    service: Rc<S>,
}

impl<S> Service<ServiceRequest> for CsrfCheckMiddleware<S>
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
        let is_mutating = matches!(
            *req.method(),
            Method::POST | Method::PUT | Method::DELETE | Method::PATCH
        );

        if is_mutating {
            let has_header = req
                .headers()
                .get(HEADER_NAME)
                .and_then(|v| v.to_str().ok())
                .is_some_and(|v| v.eq_ignore_ascii_case(EXPECTED_VALUE));

            if !has_header {
                let resp = HttpResponse::Forbidden()
                    .json(serde_json::json!({"error": "Missing CSRF header"}));
                return Box::pin(async move { Ok(req.into_response(resp)) });
            }
        }

        let srv = Rc::clone(&self.service);
        Box::pin(async move { srv.call(req).await })
    }
}

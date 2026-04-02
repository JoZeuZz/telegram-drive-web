use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::header::HeaderValue;
use actix_web::{Error, HttpMessage};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;

const HEADER_NAME: &str = "x-request-id";

/// Middleware that assigns a unique request ID to every request.
/// Sets it as both a request extension and a response header.
pub struct RequestId;

impl<S, B> Transform<S, ServiceRequest> for RequestId
where
    S: Service<ServiceRequest, Response = ServiceResponse<EitherBody<B>>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = RequestIdMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RequestIdMiddleware {
            service: Rc::new(service),
        })
    }
}

pub struct RequestIdMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for RequestIdMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<EitherBody<B>>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        ctx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Reuse incoming header or generate a new UUID
        let request_id = req
            .headers()
            .get(HEADER_NAME)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

        // Store in request extensions so handlers can access it
        req.extensions_mut()
            .insert(RequestIdValue(request_id.clone()));

        let srv = Rc::clone(&self.service);
        Box::pin(async move {
            let mut res = srv.call(req).await?;
            if let Ok(val) = HeaderValue::from_str(&request_id) {
                res.headers_mut().insert(
                    actix_web::http::header::HeaderName::from_static(HEADER_NAME),
                    val,
                );
            }
            Ok(res)
        })
    }
}

/// Newtype stored in request extensions.
#[derive(Clone)]
pub struct RequestIdValue(pub String);

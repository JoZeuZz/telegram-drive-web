use crate::http::middleware::peer_ip::extract_peer_ip;
use actix_web::body::BoxBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::Error;
use futures::future::{ok, LocalBoxFuture, Ready};
use std::rc::Rc;

/// Post-response middleware that emits structured audit log entries for
/// mutating API actions (login, logout, uploads, deletes, folder ops).
pub struct AuditLog {
    trust_proxy_headers: bool,
}

impl AuditLog {
    pub fn new(trust_proxy_headers: bool) -> Self {
        Self {
            trust_proxy_headers,
        }
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self {
            trust_proxy_headers: false,
        }
    }
}

impl<S> Transform<S, ServiceRequest> for AuditLog
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = AuditLogMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(AuditLogMiddleware {
            service: Rc::new(service),
            trust_proxy_headers: self.trust_proxy_headers,
        })
    }
}

pub struct AuditLogMiddleware<S> {
    service: Rc<S>,
    trust_proxy_headers: bool,
}

impl<S> Service<ServiceRequest> for AuditLogMiddleware<S>
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
        let method = req.method().to_string();
        let path = req.path().to_string();
        let peer = extract_peer_ip(&req, self.trust_proxy_headers);

        let srv = Rc::clone(&self.service);
        Box::pin(async move {
            let resp = srv.call(req).await?;
            let status = resp.status().as_u16();

            if let Some(action) = classify_action(&method, &path) {
                tracing::info!(
                    audit = true,
                    action = %action,
                    method = %method,
                    path = %path,
                    status = status,
                    peer = %peer,
                    "Audit event"
                );
            }

            Ok(resp)
        })
    }
}

/// Map a method+path pair to an audit action name, if noteworthy.
fn classify_action(method: &str, path: &str) -> Option<&'static str> {
    match (method, path) {
        ("POST", p) if p.ends_with("/login") => Some("login"),
        ("POST", p) if p.ends_with("/logout") => Some("logout"),
        ("POST", p) if p.ends_with("/bootstrap") => Some("password_change"),
        ("POST", p) if p.contains("/files/upload") => Some("file_upload"),
        ("DELETE", p) if p.starts_with("/api/files/") => Some("file_delete"),
        ("POST", p) if p.contains("/files/move") => Some("file_move"),
        ("POST", p) if p.starts_with("/api/folders") && !p.contains("/sync") => {
            Some("folder_create")
        }
        ("DELETE", p) if p.starts_with("/api/folders/") => Some("folder_delete"),
        ("POST", p) if p.contains("/clean-cache") => Some("cache_clean"),
        _ => None,
    }
}

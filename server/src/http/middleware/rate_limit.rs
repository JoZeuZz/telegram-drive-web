use crate::http::middleware::peer_ip::extract_peer_ip;
use actix_web::body::BoxBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::StatusCode;
use actix_web::{Error, HttpResponse};
use futures::future::{ok, LocalBoxFuture, Ready};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Mutex;
use std::time::Instant;

/// Simple in-process rate limiter middleware (token-bucket per IP).
///
/// `max_requests` tokens are refilled over `window_secs`.
/// When exhausted, the client receives 429 Too Many Requests.
pub struct RateLimit {
    max_requests: u32,
    window_secs: u64,
    trust_proxy_headers: bool,
}

impl RateLimit {
    pub fn new(max_requests: u32, window_secs: u64) -> Self {
        Self {
            max_requests,
            window_secs,
            trust_proxy_headers: false,
        }
    }

    pub fn with_trust_proxy_headers(mut self, trust_proxy_headers: bool) -> Self {
        self.trust_proxy_headers = trust_proxy_headers;
        self
    }
}

struct Bucket {
    tokens: u32,
    last_refill: Instant,
}

impl<S> Transform<S, ServiceRequest> for RateLimit
where
    S: Service<ServiceRequest, Response = ServiceResponse<BoxBody>, Error = Error> + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = RateLimitMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(RateLimitMiddleware {
            service: Rc::new(service),
            buckets: Rc::new(Mutex::new(HashMap::new())),
            max_requests: self.max_requests,
            window_secs: self.window_secs,
            trust_proxy_headers: self.trust_proxy_headers,
        })
    }
}

pub struct RateLimitMiddleware<S> {
    service: Rc<S>,
    buckets: Rc<Mutex<HashMap<String, Bucket>>>,
    max_requests: u32,
    window_secs: u64,
    trust_proxy_headers: bool,
}

impl<S> Service<ServiceRequest> for RateLimitMiddleware<S>
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
        let peer = extract_peer_ip(&req, self.trust_proxy_headers);
        let allowed = {
            let mut map = self.buckets.lock().expect("rate-limit lock poisoned");
            let now = Instant::now();

            let bucket = map.entry(peer.clone()).or_insert(Bucket {
                tokens: self.max_requests,
                last_refill: now,
            });

            // Refill tokens based on elapsed time
            let elapsed = now.duration_since(bucket.last_refill).as_secs();
            if elapsed >= self.window_secs {
                bucket.tokens = self.max_requests;
                bucket.last_refill = now;
            }

            if bucket.tokens > 0 {
                bucket.tokens -= 1;
                true
            } else {
                false
            }
        };

        if !allowed {
            tracing::warn!(peer = %peer, "Rate limit exceeded");
            let resp = HttpResponse::build(StatusCode::TOO_MANY_REQUESTS)
                .json(serde_json::json!({ "error": "Too many requests. Try again later." }));
            return Box::pin(async move { Ok(req.into_response(resp)) });
        }

        let svc = self.service.clone();
        Box::pin(async move { svc.call(req).await })
    }
}

/// Extract the client IP, preferring X-Forwarded-For (set by the reverse proxy).
#[cfg(test)]
fn peer_ip(req: &ServiceRequest, trust_proxy_headers: bool) -> String {
    extract_peer_ip(req, trust_proxy_headers)
}

#[cfg(test)]
mod tests {
    use super::peer_ip;
    use actix_web::test::TestRequest;
    use std::net::SocketAddr;

    #[test]
    fn peer_ip_uses_socket_address_when_proxy_not_trusted() {
        let req = TestRequest::default()
            .insert_header(("x-forwarded-for", "203.0.113.7, 10.0.0.1"))
            .peer_addr("198.51.100.9:12345".parse::<SocketAddr>().unwrap())
            .to_srv_request();

        assert_eq!(peer_ip(&req, false), "198.51.100.9");
    }

    #[test]
    fn peer_ip_uses_x_forwarded_for_when_proxy_is_trusted() {
        let req = TestRequest::default()
            .insert_header(("x-forwarded-for", "203.0.113.7, 10.0.0.1"))
            .peer_addr("198.51.100.9:12345".parse::<SocketAddr>().unwrap())
            .to_srv_request();

        assert_eq!(peer_ip(&req, true), "203.0.113.7");
    }
}

use actix_web::dev::ServiceRequest;

/// Extract the client IP from the request.
///
/// When `trust_proxy_headers` is true, prefers `X-Forwarded-For` first hop.
/// Otherwise falls back to the direct socket peer address.
pub fn extract_peer_ip(req: &ServiceRequest, trust_proxy_headers: bool) -> String {
    if trust_proxy_headers {
        if let Some(forwarded) = req
            .headers()
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty() && *s != "unknown")
        {
            return forwarded.to_string();
        }
    }

    req.peer_addr()
        .map(|addr| addr.ip().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

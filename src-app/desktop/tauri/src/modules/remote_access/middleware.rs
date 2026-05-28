//! Localhost-only middleware for the remote_access endpoints.
//!
//! Defense in depth on top of the `RequirePermissions<(RemoteAccessManage,)>`
//! check: even if a phone user has somehow obtained an admin JWT, they
//! cannot disable the tunnel they're using or change the ngrok auth
//! token from a tunneled request — ngrok preserves the original
//! `Host` header (`my-app.ngrok.app` etc.) by default, so the middleware
//! rejects with 403 when `Host` is anything other than 127.0.0.1 or
//! localhost. The Tauri webview always calls `http://127.0.0.1:PORT/...`
//! and passes the check trivially.

use axum::{
    body::Body,
    extract::Request,
    http::{HeaderMap, StatusCode, header},
    middleware::Next,
    response::Response,
};

/// Tower middleware (axum 0.8 from_fn style) that rejects requests
/// whose `Host` header is not a loopback address.
///
/// 403 + a clear error body lets the desktop UI surface the violation
/// in case of a misconfigured proxy; under normal operation the Tauri
/// webview should never trip this.
pub async fn require_localhost_host(req: Request<Body>, next: Next) -> Response {
    if !is_localhost_host(req.headers()) {
        tracing::warn!(
            "remote_access: rejecting non-localhost request (host={:?}, path={})",
            req.headers().get(header::HOST).and_then(|v| v.to_str().ok()),
            req.uri().path()
        );
        return Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(
                r#"{"error":{"code":"REMOTE_ACCESS_NON_LOCALHOST","message":"Remote-access configuration endpoints are only reachable from localhost. Use the desktop app to manage these settings."}}"#,
            ))
            .unwrap();
    }
    next.run(req).await
}

/// True if the request `Host` header maps to a loopback address.
/// Accepts `127.0.0.1[:port]`, `[::1][:port]`, `localhost[:port]`.
/// Missing Host → reject (don't fall back to "trust" because that
/// would let a stripped-Host request through).
pub fn is_localhost_host(headers: &HeaderMap) -> bool {
    let Some(host_value) = headers.get(header::HOST).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    host_value_is_localhost(host_value)
}

/// String-only form, exposed for unit tests.
pub fn host_value_is_localhost(host_value: &str) -> bool {
    if host_value.is_empty() {
        return false;
    }
    // Strip the port if present. Bracketed IPv6: "[::1]:8080" → "[::1]".
    let host_only = if let Some(stripped) = host_value.strip_prefix('[') {
        // IPv6 form: [::1]:port → ::1
        match stripped.find(']') {
            Some(close) => &stripped[..close],
            None => return false, // malformed
        }
    } else {
        host_value.split(':').next().unwrap_or("")
    };
    let lowered = host_only.to_ascii_lowercase();
    matches!(lowered.as_str(), "127.0.0.1" | "::1" | "localhost")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_127_0_0_1_with_port() {
        assert!(host_value_is_localhost("127.0.0.1:8080"));
    }

    #[test]
    fn accepts_127_0_0_1_without_port() {
        assert!(host_value_is_localhost("127.0.0.1"));
    }

    #[test]
    fn accepts_localhost_with_port() {
        assert!(host_value_is_localhost("localhost:8080"));
    }

    #[test]
    fn accepts_localhost_case_insensitive() {
        assert!(host_value_is_localhost("LocalHost:8080"));
    }

    #[test]
    fn accepts_ipv6_loopback_bracketed() {
        assert!(host_value_is_localhost("[::1]:8080"));
        assert!(host_value_is_localhost("[::1]"));
    }

    #[test]
    fn rejects_ngrok_host() {
        assert!(!host_value_is_localhost("my-app.ngrok.app"));
        assert!(!host_value_is_localhost("abc123.ngrok-free.app"));
    }

    #[test]
    fn rejects_arbitrary_lan_ip() {
        assert!(!host_value_is_localhost("192.168.1.10:8080"));
        assert!(!host_value_is_localhost("10.0.0.5"));
    }

    #[test]
    fn rejects_empty() {
        assert!(!host_value_is_localhost(""));
    }

    #[test]
    fn rejects_malformed_ipv6() {
        assert!(!host_value_is_localhost("[::1"));
    }
}

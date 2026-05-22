//! OAuth 2.1 client for external HTTP MCP servers (Plan-3 Phase-4, Cos1).
//!
//! Implements the **headless `client_credentials`** grant — the simplest flow
//! that needs no interactive redirect/PKCE (server-to-server auth). The shape
//! mirrors the MCP TypeScript SDK's `ClientCredentialsProvider`:
//!
//! 1. A request to the MCP server returns **401** with a
//!    `WWW-Authenticate: Bearer resource_metadata="…", scope="…"` header.
//! 2. Fetch the Protected-Resource Metadata (RFC 9728) at the advertised URL →
//!    `authorization_servers[]`.
//! 3. Discover the Authorization-Server metadata (try
//!    `/.well-known/oauth-authorization-server`, then
//!    `/.well-known/openid-configuration`) → `token_endpoint`.
//! 4. `POST <token_endpoint>` with HTTP Basic `client_id:client_secret`,
//!    `grant_type=client_credentials` (+ optional `scope`, `resource`).
//! 5. Store the `access_token`; the caller retries the original request once
//!    with `Authorization: Bearer <token>`.
//!
//! Refresh: when a `refresh_token` was issued, `grant_type=refresh_token` is
//! tried before falling back to a fresh `client_credentials` exchange.
//!
//! Interactive `authorization_code` + PKCE is intentionally out of scope here
//! (it needs a redirect URL + user consent UI); this covers the headless case
//! configured per-server with a client id/secret.

use base64::Engine;
use reqwest::Client;
use serde::Deserialize;
use std::time::{Duration, Instant};

use crate::common::AppError;

/// Per-server OAuth client_credentials configuration (sourced from the MCP
/// server config). Only present when the server is configured for OAuth.
#[derive(Debug, Clone)]
pub struct OAuthClientConfig {
    pub client_id: String,
    pub client_secret: String,
    /// Space-separated OAuth scopes, if the server requires specific ones.
    pub scopes: Option<String>,
    /// RFC 8707 resource indicator — usually the MCP server's own URL. Lets the
    /// authorization server audience-bind the token to this resource.
    pub resource: Option<String>,
}

/// A token obtained from the authorization server, with a computed expiry so
/// the client can refresh proactively rather than waiting for a 401.
#[derive(Debug, Clone)]
pub struct StoredToken {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Absolute instant after which the token should be considered expired.
    /// `None` means the server didn't send `expires_in` (treat as long-lived).
    pub expires_at: Option<Instant>,
}

impl StoredToken {
    /// Is the token still usable? Applies a 30s safety skew so we refresh just
    /// before the real expiry rather than racing it.
    pub fn is_valid(&self) -> bool {
        match self.expires_at {
            Some(exp) => Instant::now() + Duration::from_secs(30) < exp,
            None => true,
        }
    }
}

/// Parsed `WWW-Authenticate: Bearer …` challenge.
#[derive(Debug, Default, Clone)]
pub struct WwwAuthenticate {
    pub resource_metadata: Option<String>,
    pub scope: Option<String>,
}

/// Parse the `WWW-Authenticate` header value for the `Bearer` scheme params we
/// care about (`resource_metadata`, `scope`). Tolerant of quoting/spacing.
pub fn parse_www_authenticate(header: &str) -> WwwAuthenticate {
    let mut out = WwwAuthenticate::default();
    // Strip a leading scheme token ("Bearer ") if present.
    let params = header
        .strip_prefix("Bearer")
        .or_else(|| header.strip_prefix("bearer"))
        .unwrap_or(header)
        .trim();
    for part in params.split(',') {
        let part = part.trim();
        if let Some((k, v)) = part.split_once('=') {
            let k = k.trim().to_ascii_lowercase();
            let v = v.trim().trim_matches('"').to_string();
            match k.as_str() {
                "resource_metadata" => out.resource_metadata = Some(v),
                "scope" => out.scope = Some(v),
                _ => {}
            }
        }
    }
    out
}

#[derive(Debug, Deserialize)]
struct ProtectedResourceMetadata {
    #[serde(default)]
    authorization_servers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct AuthServerMetadata {
    token_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
}

/// Fetch the RFC 9728 Protected-Resource Metadata and return the first
/// advertised authorization server base URL.
async fn discover_authorization_server_base(
    client: &Client,
    resource_metadata_url: &str,
) -> Result<String, AppError> {
    let resp = client
        .get(resource_metadata_url)
        .header("Accept", "application/json")
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("OAuth PRM fetch failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(AppError::internal_error(format!(
            "OAuth PRM fetch returned HTTP {}",
            resp.status()
        )));
    }
    let prm: ProtectedResourceMetadata = resp
        .json()
        .await
        .map_err(|e| AppError::internal_error(format!("OAuth PRM parse failed: {e}")))?;
    prm.authorization_servers
        .into_iter()
        .next()
        .ok_or_else(|| AppError::internal_error("OAuth PRM lists no authorization_servers"))
}

/// Discover the token endpoint from an authorization-server base URL. Tries the
/// OAuth and OpenID well-known documents in turn (spec § AS metadata discovery).
async fn discover_token_endpoint(client: &Client, as_base: &str) -> Result<String, AppError> {
    let base = as_base.trim_end_matches('/');
    let candidates = [
        format!("{base}/.well-known/oauth-authorization-server"),
        format!("{base}/.well-known/openid-configuration"),
    ];
    for url in candidates.iter() {
        let resp = match client.get(url).header("Accept", "application/json").send().await {
            Ok(r) if r.status().is_success() => r,
            _ => continue,
        };
        if let Ok(md) = resp.json::<AuthServerMetadata>().await {
            if let Some(ep) = md.token_endpoint {
                return Ok(ep);
            }
        }
    }
    // Last-resort default per RFC 8414 (token endpoint at /token).
    Ok(format!("{base}/token"))
}

/// `POST <token_endpoint>` with a `client_credentials` grant and HTTP Basic
/// client authentication. Returns the stored token with computed expiry.
async fn request_client_credentials_token(
    client: &Client,
    token_endpoint: &str,
    config: &OAuthClientConfig,
) -> Result<StoredToken, AppError> {
    let mut form = vec![("grant_type", "client_credentials".to_string())];
    if let Some(scope) = &config.scopes {
        if !scope.is_empty() {
            form.push(("scope", scope.clone()));
        }
    }
    if let Some(resource) = &config.resource {
        form.push(("resource", resource.clone()));
    }

    let basic = base64::engine::general_purpose::STANDARD
        .encode(format!("{}:{}", config.client_id, config.client_secret));

    let resp = client
        .post(token_endpoint)
        .header("Authorization", format!("Basic {basic}"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .map_err(|e| AppError::internal_error(format!("OAuth token request failed: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::internal_error(format!(
            "OAuth token endpoint returned HTTP {status}: {}",
            body.chars().take(200).collect::<String>()
        )));
    }
    let tok: TokenResponse = resp
        .json()
        .await
        .map_err(|e| AppError::internal_error(format!("OAuth token parse failed: {e}")))?;

    Ok(StoredToken {
        access_token: tok.access_token,
        refresh_token: tok.refresh_token,
        expires_at: tok.expires_in.map(|s| Instant::now() + Duration::from_secs(s)),
    })
}

/// Full discovery + token acquisition driven by a `WWW-Authenticate` challenge.
/// Used on the first 401 from the MCP server.
/// Returns the acquired token **and** the discovered token endpoint (the
/// caller caches the endpoint so a later refresh skips re-discovery).
pub async fn obtain_token_from_challenge(
    client: &Client,
    www_authenticate: &str,
    config: &OAuthClientConfig,
) -> Result<(StoredToken, String), AppError> {
    let challenge = parse_www_authenticate(www_authenticate);
    let resource_metadata = challenge.resource_metadata.ok_or_else(|| {
        AppError::internal_error(
            "OAuth challenge missing resource_metadata; cannot discover authorization server",
        )
    })?;
    let as_base = discover_authorization_server_base(client, &resource_metadata).await?;
    let token_endpoint = discover_token_endpoint(client, &as_base).await?;
    let token = request_client_credentials_token(client, &token_endpoint, config).await?;
    Ok((token, token_endpoint))
}

/// Refresh an existing token. Uses `grant_type=refresh_token` when a refresh
/// token is available; otherwise re-runs the discovery-driven client-credentials
/// exchange. `token_endpoint` is remembered from the initial acquisition.
pub async fn refresh_token(
    client: &Client,
    token_endpoint: &str,
    config: &OAuthClientConfig,
    current: &StoredToken,
) -> Result<StoredToken, AppError> {
    if let Some(refresh) = &current.refresh_token {
        let basic = base64::engine::general_purpose::STANDARD
            .encode(format!("{}:{}", config.client_id, config.client_secret));
        let form = vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh.clone()),
        ];
        let resp = client
            .post(token_endpoint)
            .header("Authorization", format!("Basic {basic}"))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .header("Accept", "application/json")
            .form(&form)
            .send()
            .await
            .map_err(|e| AppError::internal_error(format!("OAuth refresh failed: {e}")))?;
        if resp.status().is_success() {
            if let Ok(tok) = resp.json::<TokenResponse>().await {
                return Ok(StoredToken {
                    access_token: tok.access_token,
                    refresh_token: tok.refresh_token.or_else(|| current.refresh_token.clone()),
                    expires_at: tok.expires_in.map(|s| Instant::now() + Duration::from_secs(s)),
                });
            }
        }
        // Fall through to a fresh client-credentials exchange on refresh failure.
    }
    request_client_credentials_token(client, token_endpoint, config).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_www_authenticate_with_quotes_and_spaces() {
        let h = r#"Bearer resource_metadata="https://srv/.well-known/oauth-protected-resource", scope="mcp read""#;
        let p = parse_www_authenticate(h);
        assert_eq!(
            p.resource_metadata.as_deref(),
            Some("https://srv/.well-known/oauth-protected-resource")
        );
        assert_eq!(p.scope.as_deref(), Some("mcp read"));
    }

    #[test]
    fn parses_www_authenticate_lowercase_scheme_no_quotes() {
        let p = parse_www_authenticate("bearer resource_metadata=https://x/prm");
        assert_eq!(p.resource_metadata.as_deref(), Some("https://x/prm"));
        assert!(p.scope.is_none());
    }

    #[test]
    fn token_validity_respects_skew() {
        let expired = StoredToken {
            access_token: "t".into(),
            refresh_token: None,
            expires_at: Some(Instant::now() + Duration::from_secs(10)), // < 30s skew
        };
        assert!(!expired.is_valid(), "token within the 30s skew is treated as expired");
        let fresh = StoredToken {
            access_token: "t".into(),
            refresh_token: None,
            expires_at: Some(Instant::now() + Duration::from_secs(120)),
        };
        assert!(fresh.is_valid());
        let no_expiry = StoredToken {
            access_token: "t".into(),
            refresh_token: None,
            expires_at: None,
        };
        assert!(no_expiry.is_valid());
    }
}

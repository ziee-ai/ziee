//! Tunnel-driver abstraction + ngrok impl + mock impl.
//!
//! We hide the actual ngrok session behind a `TunnelDriver` trait so
//! the integration tests can substitute a `MockTunnelDriver` and run
//! the full request flow (start → status → stop) without any
//! network. The prod implementation (`NgrokDriver`) wraps the
//! `ngrok` crate.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use std::sync::Arc;
use tokio::sync::RwLock;

use ziee::AppError;

use super::models::TunnelStateKind;

/// What the handler needs to know about the live tunnel.
#[derive(Debug, Clone)]
pub struct TunnelStatus {
    pub state: TunnelStateKind,
    pub public_url: Option<String>,
    pub last_error: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
}

impl TunnelStatus {
    pub fn idle() -> Self {
        Self {
            state: TunnelStateKind::Idle,
            public_url: None,
            last_error: None,
            started_at: None,
        }
    }
}

/// Why a start attempt failed (or why "already running" is a fine
/// outcome rather than an error).
#[derive(Debug, thiserror::Error)]
pub enum TunnelError {
    #[error("tunnel is already running")]
    AlreadyRunning,
    #[error("tunnel auth failed: {0}")]
    AuthFailed(String),
    #[error("tunnel error: {0}")]
    Other(String),
}

#[async_trait]
pub trait TunnelDriver: Send + Sync + 'static {
    /// Start the tunnel against `target_port` (localhost). `auth_token`
    /// is the ngrok account token. `domain` is an optional reserved /
    /// custom domain (None → ngrok auto-assigns).
    async fn start(
        &self,
        auth_token: &str,
        domain: Option<&str>,
        target_port: u16,
    ) -> Result<String, TunnelError>;

    /// Stop a running tunnel (idempotent).
    async fn stop(&self) -> Result<(), TunnelError>;

    /// Current status snapshot.
    async fn status(&self) -> TunnelStatus;
}

/// Shared state stored in the Axum app extensions. Wraps an
/// `Arc<dyn TunnelDriver>` so the prod / mock split lives here.
#[derive(Clone)]
pub struct TunnelDriverHandle(pub Arc<dyn TunnelDriver>);

impl TunnelDriverHandle {
    pub fn new(driver: Arc<dyn TunnelDriver>) -> Self {
        Self(driver)
    }
}

// =====================================================
// Mock implementation (test-only)
// =====================================================

/// In-memory tunnel driver for integration tests. Doesn't talk to
/// any external service; just flips state and returns a canned URL.
pub struct MockTunnelDriver {
    state: RwLock<TunnelStatus>,
    /// URL pattern returned on start. Tests can read `state.public_url`
    /// after start to see what was minted.
    url_template: String,
}

impl MockTunnelDriver {
    pub fn new() -> Self {
        Self {
            state: RwLock::new(TunnelStatus::idle()),
            url_template: "https://mock-{stamp}.ngrok-mock.test".to_string(),
        }
    }
}

impl Default for MockTunnelDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TunnelDriver for MockTunnelDriver {
    async fn start(
        &self,
        _auth_token: &str,
        domain: Option<&str>,
        _target_port: u16,
    ) -> Result<String, TunnelError> {
        let mut state = self.state.write().await;
        if state.state == TunnelStateKind::Connected || state.state == TunnelStateKind::Starting {
            return Err(TunnelError::AlreadyRunning);
        }
        let now = Utc::now();
        let url = match domain {
            Some(d) => format!("https://{}", d),
            None => self.url_template.replace("{stamp}", &now.timestamp().to_string()),
        };
        state.state = TunnelStateKind::Connected;
        state.public_url = Some(url.clone());
        state.last_error = None;
        state.started_at = Some(now);
        Ok(url)
    }

    async fn stop(&self) -> Result<(), TunnelError> {
        let mut state = self.state.write().await;
        *state = TunnelStatus::idle();
        Ok(())
    }

    async fn status(&self) -> TunnelStatus {
        self.state.read().await.clone()
    }
}

// =====================================================
// ngrok implementation (prod)
// =====================================================

/// Production tunnel driver backed by the `ngrok` Rust crate.
///
/// Lifecycle:
///   - `start()` builds a Session, opens an HTTP endpoint with the
///     optional domain, kicks off a background forwarder to
///     `127.0.0.1:target_port`, and stores the tunnel handle in
///     `inner.handle`. Dropping the handle terminates the tunnel.
///   - `stop()` drops the stored handle (and any background task).
///   - `status()` returns the cached state.
pub struct NgrokDriver {
    inner: Arc<RwLock<NgrokDriverInner>>,
    /// Serializes `start`/`stop` so concurrent calls can't both pass
    /// the "AlreadyRunning?" check and end up racing two `start_inner`
    /// invocations against ngrok (which would leak the loser's
    /// session and overwrite its handle). The status `RwLock` above
    /// is held only briefly inside the critical section; this
    /// operation `Mutex` is held across the long ngrok I/O.
    op_lock: Arc<tokio::sync::Mutex<()>>,
}

struct NgrokDriverInner {
    status: TunnelStatus,
    /// Live tunnel handle. Dropping it terminates the tunnel.
    /// `Option<Box<dyn Any + Send>>` so this module compiles even
    /// when the `ngrok` crate is feature-gated away — the actual
    /// downcast happens only inside the (feature-flagged) start path.
    #[allow(dead_code)]
    handle: Option<NgrokHandle>,
}

/// Opaque wrapper around the live ngrok tunnel + forwarder task.
/// The forwarder task owns the `ngrok::tunnel::HttpTunnel`; aborting
/// the task drops the tunnel which closes the ngrok session.
pub struct NgrokHandle {
    forwarder: tokio::task::JoinHandle<()>,
}

impl Drop for NgrokHandle {
    fn drop(&mut self) {
        // Abort the forwarder task; the owned ngrok tunnel drops with
        // the task and closes the session cleanly.
        self.forwarder.abort();
    }
}

impl NgrokDriver {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(NgrokDriverInner {
                status: TunnelStatus::idle(),
                handle: None,
            })),
            op_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

impl Default for NgrokDriver {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TunnelDriver for NgrokDriver {
    async fn start(
        &self,
        auth_token: &str,
        domain: Option<&str>,
        target_port: u16,
    ) -> Result<String, TunnelError> {
        // Hold the op_lock across the entire start so concurrent
        // start/stop calls serialize. Other readers (status polling)
        // are not blocked — they go through `inner` RwLock.
        let _op = self.op_lock.lock().await;

        // Atomic check-and-flip under the inner write lock.
        {
            let mut inner = self.inner.write().await;
            if matches!(
                inner.status.state,
                TunnelStateKind::Connected | TunnelStateKind::Starting
            ) {
                return Err(TunnelError::AlreadyRunning);
            }
            inner.status.state = TunnelStateKind::Starting;
            inner.status.last_error = None;
        }

        let result = self
            .start_inner(auth_token, domain, target_port)
            .await;

        let mut inner = self.inner.write().await;
        match result {
            Ok((url, handle)) => {
                inner.status.state = TunnelStateKind::Connected;
                inner.status.public_url = Some(url.clone());
                inner.status.started_at = Some(Utc::now());
                inner.handle = Some(handle);
                Ok(url)
            }
            Err(e) => {
                inner.status.state = TunnelStateKind::Error;
                inner.status.last_error = Some(e.to_string());
                Err(e)
            }
        }
    }

    async fn stop(&self) -> Result<(), TunnelError> {
        // Serialize with start; if a start is in flight, wait for it
        // to settle before tearing down — avoids dropping a half-built
        // handle out from under start_inner's final write.
        let _op = self.op_lock.lock().await;
        let mut inner = self.inner.write().await;
        // Dropping the handle aborts the forwarder + drops the tunnel.
        inner.handle = None;
        inner.status = TunnelStatus::idle();
        Ok(())
    }

    async fn status(&self) -> TunnelStatus {
        self.inner.read().await.status.clone()
    }
}

impl NgrokDriver {
    /// Build the ngrok session + open the endpoint + spawn forwarder.
    /// Returns `(public_url, handle)`. Wrapped so the outer `start`
    /// can centralize state-machine bookkeeping.
    async fn start_inner(
        &self,
        auth_token: &str,
        domain: Option<&str>,
        target_port: u16,
    ) -> Result<(String, NgrokHandle), TunnelError> {
        use ngrok::prelude::*;
        use std::str::FromStr;

        // ngrok's `Session::connect` builds a rustls `ClientConfig`
        // internally. rustls 0.23 panics ("Could not automatically
        // determine the process-level CryptoProvider") when more than one
        // provider is linked — which is exactly our case: `aws-lc-rs` (via
        // reqwest's rustls-tls) AND `ring` (via ldap3's tls-rustls-ring)
        // are both in the graph, so rustls can't auto-pick. Install one
        // explicitly, process-wide, before any rustls config is built.
        // Idempotent: `install_default` returns Err once a provider is
        // already set, which we ignore; the `Once` keeps it race-free.
        static CRYPTO_PROVIDER_INIT: std::sync::Once = std::sync::Once::new();
        CRYPTO_PROVIDER_INIT.call_once(|| {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        });

        let sess = ngrok::Session::builder()
            .authtoken(auth_token)
            .connect()
            .await
            .map_err(|e| TunnelError::AuthFailed(e.to_string()))?;

        // ngrok 0.14: `http_endpoint()` returns an `HttpTunnelBuilder`.
        // The `.domain()` builder method takes `&mut self` and returns
        // `&mut Self` (chainable in-place), so apply it to the mutable
        // binding rather than rebinding.
        let mut http_endpoint = sess.http_endpoint();
        if let Some(d) = domain {
            http_endpoint.domain(d);
        }
        let mut tunnel = http_endpoint
            .listen()
            .await
            .map_err(|e| TunnelError::Other(e.to_string()))?;

        let public_url = tunnel.url().to_string();
        // Build the local forward URL. ngrok accepts a `url::Url`.
        let forward_url = url::Url::from_str(&format!("http://127.0.0.1:{}", target_port))
            .map_err(|e| TunnelError::Other(format!("bad forward URL: {}", e)))?;

        // Background task: forward all incoming ngrok connections to
        // the local server's HTTP listener. Drops on tunnel close.
        // When the forwarder exits unexpectedly (network drop, ngrok
        // kicks us off, idle timeout), flip the driver status to
        // `Error` so the UI can surface it — otherwise the page keeps
        // claiming "connected" forever even though the tunnel is dead.
        let status_inner = Arc::clone(&self.inner);
        let forwarder = tokio::spawn(async move {
            #[allow(deprecated)]
            let result = tunnel.forward(forward_url).await;
            match result {
                Ok(()) => {
                    tracing::info!("ngrok forwarder ended cleanly (tunnel closed)");
                }
                Err(e) => {
                    tracing::warn!("ngrok forwarder ended with error: {}", e);
                    let mut inner = status_inner.write().await;
                    // Only flip to Error if the status still claims
                    // we're connected — if stop() already ran (which
                    // sets state to Idle and drops the handle), don't
                    // overwrite the idle state with an error.
                    if matches!(inner.status.state, TunnelStateKind::Connected) {
                        inner.status.state = TunnelStateKind::Error;
                        inner.status.last_error = Some(format!("forwarder ended: {}", e));
                        inner.status.public_url = None;
                        inner.status.started_at = None;
                        inner.handle = None;
                    }
                }
            }
        });

        Ok((public_url, NgrokHandle { forwarder }))
    }
}

/// Maps a TunnelError to AppError + status code for the handlers.
pub fn tunnel_error_to_api(err: TunnelError) -> (axum::http::StatusCode, AppError) {
    use axum::http::StatusCode;
    match err {
        TunnelError::AlreadyRunning => (
            StatusCode::CONFLICT,
            AppError::new(
                StatusCode::CONFLICT,
                "TUNNEL_ALREADY_RUNNING",
                "Tunnel is already running. Stop it before starting again.",
            ),
        ),
        TunnelError::AuthFailed(msg) => {
            // Keep the raw ngrok detail in logs; surface a clean
            // user-friendly message via the API. Raw ngrok errors can
            // include internal hostnames / session IDs that we don't
            // want bleeding through to UIs or shared screenshots.
            tracing::warn!(detail = %msg, "remote_access: ngrok auth failed");
            (
                StatusCode::UNPROCESSABLE_ENTITY,
                AppError::new(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "TUNNEL_AUTH_FAILED",
                    "ngrok rejected the auth token. Verify it on dashboard.ngrok.com (Account → Your Authtoken).",
                ),
            )
        }
        TunnelError::Other(msg) => {
            tracing::warn!(detail = %msg, "remote_access: tunnel error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                AppError::internal_error(format!("tunnel error: {}", msg)),
            )
        }
    }
}

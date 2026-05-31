//! Process-global handles for the remote_access module.
//!
//! Two shared handles live here:
//!   - `TunnelDriverHandle` — the active tunnel driver (NgrokDriver
//!     in prod, MockTunnelDriver in tests). Set by the module's
//!     `init()` and read by handlers + auto_start_if_configured().
//!   - The local server's bound port — needed when starting the
//!     tunnel so ngrok knows where to forward to. Provided by the
//!     existing `ServerConfig.port` via a global setter.
//!
//! Test injection: setting the env var
//! `ZIEE_REMOTE_ACCESS_MOCK_TUNNEL=1` BEFORE the first call to
//! `tunnel_driver()` makes the default driver a `MockTunnelDriver`
//! instead of `NgrokDriver`. The Tier-3 integration test
//! (`tunnel_start_mock_success`) uses this to exercise the full
//! happy-path start → public_url flow without touching ngrok's edge.

use std::sync::{Arc, OnceLock};

use super::tunnel::{MockTunnelDriver, NgrokDriver, TunnelDriver, TunnelDriverHandle};

static TUNNEL_DRIVER: OnceLock<TunnelDriverHandle> = OnceLock::new();
static LOCAL_SERVER_PORT: OnceLock<u16> = OnceLock::new();

/// Env-var sentinel: when set to any non-empty value before the
/// first `tunnel_driver()` call, the default driver becomes
/// `MockTunnelDriver` instead of `NgrokDriver`. Used by Tier-3
/// integration tests; physically compiled out of release behaviour
/// only insofar as nothing in prod ever sets the var.
pub const MOCK_TUNNEL_ENV: &str = "ZIEE_REMOTE_ACCESS_MOCK_TUNNEL";

/// Initialize the tunnel driver. Idempotent: first caller wins.
/// In prod main.rs calls this with `NgrokDriver::new()`; integration
/// tests can call it directly OR rely on the env-var fallback in
/// `tunnel_driver()` below.
pub fn init_tunnel_driver(driver: Arc<dyn TunnelDriver>) {
    if TUNNEL_DRIVER
        .set(TunnelDriverHandle::new(driver))
        .is_err()
    {
        tracing::warn!(
            "remote_access: init_tunnel_driver called more than once \
             (or after tunnel_driver() lazy-init); explicit driver ignored"
        );
    }
}

/// Returns the active tunnel driver, defaulting to either the mock
/// (when `ZIEE_REMOTE_ACCESS_MOCK_TUNNEL` is set) or `NgrokDriver`
/// otherwise. Prod main.rs SHOULD still call `init_tunnel_driver`
/// explicitly so the choice is auditable; the default exists for
/// test ergonomics and so handlers don't have to handle a missing
/// driver.
pub fn tunnel_driver() -> TunnelDriverHandle {
    TUNNEL_DRIVER
        .get_or_init(|| {
            let mock_requested = std::env::var(MOCK_TUNNEL_ENV)
                .map(|v| !v.is_empty() && v != "0" && v.to_ascii_lowercase() != "false")
                .unwrap_or(false);
            if mock_requested {
                tracing::info!(
                    "remote_access: {} set; using MockTunnelDriver (NO real ngrok session)",
                    MOCK_TUNNEL_ENV
                );
                TunnelDriverHandle::new(Arc::new(MockTunnelDriver::new()))
            } else {
                TunnelDriverHandle::new(Arc::new(NgrokDriver::new()))
            }
        })
        .clone()
}

/// Record the local HTTP server's bound port. ngrok needs this so it
/// can forward inbound tunnel traffic to the right local socket.
pub fn set_local_server_port(port: u16) {
    if LOCAL_SERVER_PORT.set(port).is_err() {
        // Pre-existing value wins. A second call typically indicates
        // a second backend boot in the same process (e.g. tests);
        // silently inheriting the first port would tunnel to the
        // wrong upstream, so we surface it loudly.
        let prior = LOCAL_SERVER_PORT.get().copied().unwrap_or(0);
        tracing::warn!(
            requested = port,
            prior,
            "remote_access: set_local_server_port called more than once; prior value kept"
        );
    }
}

/// Retrieve the local server port; falls back to 8080 if not set
/// (only happens in degenerate test scenarios; in real builds
/// `BackendModule::init` / `run_headless` always call the setter).
pub fn local_server_port() -> u16 {
    match LOCAL_SERVER_PORT.get() {
        Some(p) => *p,
        None => {
            tracing::warn!(
                "remote_access: LOCAL_SERVER_PORT not initialized; falling back to 8080. \
                 Tunnel forwarding will be wrong unless the backend happens to be on 8080."
            );
            8080
        }
    }
}

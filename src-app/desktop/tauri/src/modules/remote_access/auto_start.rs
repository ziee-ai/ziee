//! Auto-start-on-boot wiring for the remote_access module.
//!
//! Called after the HTTP server has bound its local port (so
//! `state::set_local_server_port` is set) but before serving real
//! traffic. Reads the singleton settings row; if all three
//! conditions hold (`auto_start_tunnel`, `ngrok_domain.is_some()`,
//! `ngrok_auth_token.is_some()`) it kicks off the tunnel.
//!
//! Failures are logged at WARN; the server boots normally without
//! the tunnel rather than crash-looping.

use ziee::Repos;

use super::repository::RemoteAccessRepository;
use super::state::{local_server_port, tunnel_driver};

/// True if the settings would have us auto-start at boot. Pure
/// function for test purposes.
pub fn should_auto_start(
    auto_start_tunnel: bool,
    ngrok_domain: Option<&str>,
    ngrok_auth_token: Option<&str>,
) -> bool {
    auto_start_tunnel && ngrok_domain.is_some() && ngrok_auth_token.is_some()
}

/// Poll until `ziee::Repos` is initialized or the deadline passes.
/// Returns true on success, false on timeout.
///
/// Background: `RemoteAccessModule::init()` runs in the Tauri setup
/// callback, BEFORE the embedded server has actually booted (the
/// server's startup is itself spawned async by BackendModule). The
/// auto_start hook fires soon after — too soon, without this poll.
pub async fn wait_for_repos_ready(timeout: std::time::Duration) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    while !ziee::is_repos_initialized() {
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    true
}

/// Read settings, kick off the tunnel if all three preconditions
/// hold. Best-effort — logs and returns on any error so server boot
/// is not blocked.
pub async fn auto_start_if_configured() {
    if !wait_for_repos_ready(std::time::Duration::from_secs(60)).await {
        tracing::warn!(
            "remote_access: gave up waiting for Repos to be initialized; auto-start skipped"
        );
        return;
    }
    let repo = RemoteAccessRepository::new(Repos.pool().clone());
    let settings = match repo.get_settings().await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "remote_access: failed to read settings during auto-start probe; skipping"
            );
            return;
        }
    };

    if !should_auto_start(
        settings.auto_start_tunnel,
        settings.ngrok_domain.as_deref(),
        settings.ngrok_auth_token.as_deref(),
    ) {
        tracing::debug!(
            auto_start = settings.auto_start_tunnel,
            has_domain = settings.ngrok_domain.is_some(),
            has_token = settings.ngrok_auth_token.is_some(),
            "remote_access: auto-start preconditions not met; skipping"
        );
        return;
    }

    // If password_auth_enabled is somehow true but the admin password
    // has been reverted to bootstrap (e.g. DB restore from snapshot),
    // refuse to bring the tunnel up. update_settings normally enforces
    // this invariant on save, but the on-disk state could drift; this
    // re-check at boot is the last line of defense before exposing the
    // password-login surface with a well-known password.
    if settings.password_auth_enabled {
        match super::handlers::admin_password_rotated().await {
            Ok(true) => {}
            Ok(false) => {
                tracing::warn!(
                    "remote_access: refusing auto-start — password auth is enabled but the admin password is still the bootstrap default. Reset the admin password before re-enabling auto-start."
                );
                return;
            }
            Err((_status, e)) => {
                tracing::warn!(
                    error = %e,
                    "remote_access: failed to verify admin password rotation status during auto-start; skipping"
                );
                return;
            }
        }
    }

    let token = settings.ngrok_auth_token.unwrap();
    let domain = settings.ngrok_domain;
    let target_port = local_server_port();
    let driver = tunnel_driver();

    // Defensive: if an admin manually clicked "Start tunnel" during
    // the wait-for-repos window (or some other path already started
    // the tunnel), don't fight it. The driver's op_lock would
    // serialize and return AlreadyRunning anyway, but skipping here
    // avoids the warn log.
    let current = driver.0.status().await;
    if !matches!(current.state, super::models::TunnelStateKind::Idle) {
        tracing::info!(
            state = ?current.state,
            "remote_access: tunnel already in non-Idle state; auto-start skipped"
        );
        return;
    }

    tracing::info!(
        domain = ?domain,
        target_port,
        "remote_access: auto-starting tunnel"
    );

    match driver
        .0
        .start(&token, domain.as_deref(), target_port)
        .await
    {
        Ok(url) => {
            tracing::info!(public_url = %url, "remote_access: auto-start succeeded");
        }
        Err(e) => {
            tracing::warn!(
                error = %e,
                "remote_access: auto-start failed; tunnel not active"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_start_requires_all_three() {
        assert!(should_auto_start(true, Some("d.ngrok.app"), Some("tok")));
        assert!(!should_auto_start(false, Some("d.ngrok.app"), Some("tok")));
        assert!(!should_auto_start(true, None, Some("tok")));
        assert!(!should_auto_start(true, Some("d.ngrok.app"), None));
    }
}

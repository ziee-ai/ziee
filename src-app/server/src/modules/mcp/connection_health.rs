//! Connection-health enforcement for MCP servers.
//!
//! Three entry points share a single underlying probe (the same
//! `run_connection_test` the explicit "Test Connection" UI button
//! uses):
//!
//! 1. Update / enable flow — refuse to flip `enabled: false → true`
//!    when the new config can't connect (handler returns 400 with the
//!    failure detail; other fields in the same PUT still persist).
//! 2. Create flow — if the new server was requested with
//!    `enabled: true` and the probe fails, downgrade to
//!    `enabled: false` and return a warning so the row is preserved
//!    for the user to edit + retry.
//! 3. Boot — every enabled non-built-in MCP server is probed on
//!    server startup; failures flip to `enabled: false` automatically
//!    so users don't see broken servers in their tool lists.
//!
//! Built-in servers (filesystem, memory, code_sandbox, memory_mcp)
//! are SKIPPED — they're owned by the platform, not by user config,
//! and their reachability is the platform's responsibility.

use crate::common::AppError;
use crate::core::Repos;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use super::handlers::test_connection::run_connection_test;
use super::models::{McpServer, TransportType};

/// Structured probe failure carrying the underlying reason so the
/// caller can surface it (in the API response, in the boot log, or
/// in the UI toast).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct ProbeFailure {
    /// Human-readable reason — taken verbatim from
    /// `TestMcpConnectionResponse.message` (timeout / 401 / bad
    /// command / etc.).
    pub reason: String,
}

/// Wraps a created/updated `McpServer` with an optional connection
/// warning, used by the create handlers when the probe failed and
/// the server was auto-downgraded to `enabled: false`. `None` on
/// success (probe passed, or `enabled: false` was requested so no
/// probe ran).
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct McpServerWithHealthWarning {
    // Flattened so the response shape is `{...McpServer fields,
    // connection_warning?}` — see the rationale on
    // `LlmRepositoryWithHealthWarning` in
    // `modules/llm_repository/connection_health.rs`.
    #[serde(flatten)]
    pub server: super::models::McpServer,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connection_warning: Option<ProbeFailure>,
}

/// Create-flow enforcement. Call AFTER `Repos.mcp.create_*_server`
/// returns the persisted row. Probes when the new server is
/// `enabled: true` and not built-in; on probe failure, flips
/// `enabled: false` in the DB and returns the updated server with
/// `connection_warning` set. Built-in servers are never probed.
///
/// Records the probe outcome on the server's `last_health_check_*`
/// columns regardless of success/failure so the UI can surface
/// "last tried: …" without re-running.
pub async fn enforce_on_create(
    pool: &PgPool,
    server: super::models::McpServer,
    event_bus: &crate::core::events::EventBus,
) -> Result<McpServerWithHealthWarning, AppError> {
    // Skip the auto-disable probe for: disabled servers (nothing to
    // probe), built-in servers (owned by their modules), AND
    // `run_in_sandbox` servers. A sandboxed stdio server's connectivity
    // genuinely requires the code_sandbox runtime (lazy rootfs
    // fetch/mount + VM/bwrap spawn), which may not be ready at
    // create/enable time — probing it here would either route through an
    // un-mounted sandbox (false failure → wrong auto-disable, seen on the
    // macOS libkrun path) or, if we probed the raw command on the host,
    // false-fail any guest-only command. The real sandboxed connect on
    // first use surfaces genuine errors to the user; mirrors the
    // "Connectivity probe only — never routed through the code_sandbox"
    // rule the explicit Test Connection path already follows.
    if !server.enabled || server.is_built_in || server.run_in_sandbox {
        return Ok(McpServerWithHealthWarning {
            server,
            connection_warning: None,
        });
    }

    // Test-only escape hatch (debug builds): some E2E specs need to
    // create a fake-URL MCP server and keep `enabled=true` to exercise
    // the chip-row / status flows. The probe would normally fail and
    // auto-disable; bypassing leaves the server in the requested state.
    // Mirrors `ZIEE_DISABLE_MODEL_VALIDATION` in `llm_local_runtime`.
    // Compiled out of release builds via `cfg!(debug_assertions)` so
    // production can never silently skip the probe.
    if cfg!(debug_assertions)
        && std::env::var("ZIEE_DISABLE_MCP_HEALTH_CHECK").as_deref() == Ok("1")
    {
        tracing::warn!(
            server_id = %server.id,
            "mcp::health: ZIEE_DISABLE_MCP_HEALTH_CHECK=1 — skipping create probe",
        );
        return Ok(McpServerWithHealthWarning {
            server,
            connection_warning: None,
        });
    }

    match probe(pool, &server).await {
        Ok(()) => {
            // Don't fail the create on a record_health_check error —
            // the user's server already exists + is enabled; a
            // transient DB hiccup here shouldn't block the success
            // path. Log + continue.
            if let Err(e) = Repos.mcp.record_health_check(server.id, "healthy", None).await {
                tracing::warn!(error = ?e, server_id = %server.id, "mcp::health: failed to record healthy status (non-fatal)");
            }
            // Re-fetch so the response carries the recorded health
            // timestamp + status fields.
            let refetched = Repos
                .mcp
                .get_any_server(server.id)
                .await?
                .unwrap_or(server);
            Ok(McpServerWithHealthWarning {
                server: refetched,
                connection_warning: None,
            })
        }
        Err(failure) => {
            tracing::warn!(
                server_id = %server.id,
                reason = %failure.reason,
                "mcp::health: create-time probe failed; downgrading new server to disabled",
            );
            disable_for_health_failure(pool, server.id).await?;
            if let Err(e) = Repos
                .mcp
                .record_health_check(server.id, "unhealthy", Some(&failure.reason))
                .await
            {
                tracing::warn!(error = ?e, server_id = %server.id, "mcp::health: failed to record unhealthy status (non-fatal)");
            }
            event_bus.emit_async(
                super::events::McpServerEvent::auto_disabled(
                    server.id,
                    failure.reason.clone(),
                ),
            );
            // Re-fetch so the response carries the canonical state
            // (enabled=false, updated_at bumped, health columns
            // populated).
            let refetched = Repos
                .mcp
                .get_any_server(server.id)
                .await?
                .ok_or_else(|| AppError::internal_error("Server vanished after auto-disable"))?;
            Ok(McpServerWithHealthWarning {
                server: refetched,
                connection_warning: Some(failure),
            })
        }
    }
}

/// Update-flow enforcement. Call AFTER persisting all other fields
/// but BEFORE returning the response. When the update is an
/// enabled-transition (`old_enabled == false && new_enabled == true`)
/// the persisted state is probed; on failure the row's `enabled` is
/// forced back to false in the DB and the function returns a 400
/// `AppError` so the handler short-circuits. Other fields the
/// admin updated in the same PUT stay persisted (the user
/// explicitly chose this trade-off — partial save with a clear
/// error rather than losing every concurrent edit).
pub async fn enforce_on_update_transition(
    pool: &PgPool,
    persisted: super::models::McpServer,
    old_enabled: bool,
    event_bus: &crate::core::events::EventBus,
) -> Result<super::models::McpServer, AppError> {
    let transitioned_to_enabled = persisted.enabled && !old_enabled;
    // See enforce_on_create: `run_in_sandbox` servers are not probe-gated
    // (their connectivity needs the lazy code_sandbox runtime; the real
    // sandboxed connect surfaces genuine errors at use time).
    if !transitioned_to_enabled || persisted.is_built_in || persisted.run_in_sandbox {
        return Ok(persisted);
    }

    // Test-only escape hatch — see the matching block in
    // `enforce_on_create`. `cfg!(debug_assertions)` gates it out of
    // release builds.
    if cfg!(debug_assertions)
        && std::env::var("ZIEE_DISABLE_MCP_HEALTH_CHECK").as_deref() == Ok("1")
    {
        tracing::warn!(
            server_id = %persisted.id,
            "mcp::health: ZIEE_DISABLE_MCP_HEALTH_CHECK=1 — skipping enable-transition probe",
        );
        return Ok(persisted);
    }

    match probe(pool, &persisted).await {
        Ok(()) => {
            if let Err(e) = Repos
                .mcp
                .record_health_check(persisted.id, "healthy", None)
                .await
            {
                tracing::warn!(error = ?e, server_id = %persisted.id, "mcp::health: failed to record healthy status (non-fatal)");
            }
            // Re-fetch so the response carries the new health
            // columns; otherwise the in-memory `persisted` is stale
            // by one record_health_check tick.
            let refetched = Repos.mcp.get_any_server(persisted.id).await?.unwrap_or(persisted);
            Ok(refetched)
        }
        Err(failure) => {
            tracing::warn!(
                server_id = %persisted.id,
                reason = %failure.reason,
                "mcp::health: update-enable-transition probe failed; reverting to enabled=false",
            );
            disable_for_health_failure(pool, persisted.id).await?;
            if let Err(e) = Repos
                .mcp
                .record_health_check(persisted.id, "unhealthy", Some(&failure.reason))
                .await
            {
                tracing::warn!(error = ?e, server_id = %persisted.id, "mcp::health: failed to record unhealthy status (non-fatal)");
            }
            event_bus.emit_async(
                super::events::McpServerEvent::auto_disabled(
                    persisted.id,
                    failure.reason.clone(),
                ),
            );
            Err(AppError::bad_request(
                "MCP_ENABLE_FAILED_HEALTH_CHECK",
                format!(
                    "Other changes were saved, but the server could not \
                     be enabled because the connection probe failed: {}",
                    failure.reason
                ),
            ))
        }
    }
}

/// Probe an MCP server's connection. Returns `Ok(())` on a successful
/// `initialize` handshake; `Err(ProbeFailure)` otherwise.
///
/// Loads any stored OAuth config for HTTP servers — same resolution
/// path the explicit Test Connection button uses, so the probe sees
/// the same auth state the runtime would.
pub async fn probe(pool: &PgPool, server: &McpServer) -> Result<(), ProbeFailure> {
    // Resolve stored OAuth config for HTTP servers. Failure to read
    // the config is itself a probe failure — the runtime would fail
    // too, so the caller should see the same outcome.
    let oauth = match server.transport_type {
        TransportType::Http => match Repos.mcp.get_oauth_config(server.id).await {
            Ok(Some(cfg)) => Some(cfg.into_client_config()),
            Ok(None) => None,
            Err(e) => {
                return Err(ProbeFailure {
                    reason: format!("Failed to read OAuth config: {e}"),
                });
            }
        },
        _ => None,
    };
    // `pool` arg is reserved for future probe variants that need DB
    // access beyond the OAuth lookup (e.g. resolving runtime overrides);
    // the OAuth resolution above goes through `Repos.mcp` which holds
    // its own pool reference.
    let _ = pool;

    let response = run_connection_test(server.clone(), oauth).await;
    if response.success {
        Ok(())
    } else {
        Err(ProbeFailure {
            reason: response.message,
        })
    }
}

/// Boot-time health check. Iterates every `enabled = true` MCP server
/// that's not built-in, probes it, and flips `enabled = false` on
/// any failure. Logs each transition.
///
/// Runs as a fire-and-forget background task spawned from `mcp::init`
/// — should NOT block boot. Built-in servers are owned by their
/// respective modules (filesystem, memory_mcp, code_sandbox) and
/// don't go through this path.
///
/// No event emission here: the `EventBus` is built AFTER module
/// init, so it's not in scope at this stage. The on-save handlers
/// (which DO have access via Axum Extension) emit the AutoDisabled
/// event when they downgrade a server. UI pages re-fetch on mount,
/// so a boot-time auto-disable is visible the next time the user
/// opens the MCP servers list — no event channel needed for the
/// boot path specifically.
pub async fn run_startup_health_check(pool: PgPool) {
    // Use the passed `pool` for every DB op (list / record / disable) rather
    // than the process-global `Repos`. In production the two are the same pool
    // (`init_repositories` runs once at boot), but `run_startup_health_check`
    // takes a `pool` param and must be self-consistent: reading the server list
    // via a different pool than the one it writes disables through is a latent
    // bug. It also makes the boot path independent of `Repos` init ordering
    // (e.g. the in-process test harness, where sibling tests re-`init_repositories`
    // the global factory concurrently — this is what let the boot-recovery unit
    // test flake under a parallel `cargo test --lib`).
    let servers = match super::repository::list_enabled_for_health_check(&pool).await {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = ?e, "mcp::health: failed to list enabled servers for startup check");
            return;
        }
    };

    if servers.is_empty() {
        tracing::debug!("mcp::health: no enabled servers to probe");
        return;
    }

    tracing::info!(
        count = servers.len(),
        "mcp::health: probing enabled MCP servers at startup",
    );

    for server in servers {
        let server_id = server.id;
        let server_name = server.name.clone();
        match probe(&pool, &server).await {
            Ok(()) => {
                tracing::debug!(
                    server_id = %server_id,
                    server_name = %server_name,
                    "mcp::health: server reachable",
                );
                if let Err(e) = record_health_check_on(&pool, server_id, "healthy", None).await {
                    tracing::warn!(error = ?e, server_id = %server_id, "mcp::health: failed to record healthy status (non-fatal)");
                }
            }
            Err(failure) => {
                tracing::warn!(
                    server_id = %server_id,
                    server_name = %server_name,
                    reason = %failure.reason,
                    "mcp::health: auto-disabling unreachable MCP server",
                );
                // Best-effort flip; if the UPDATE itself fails, log
                // and keep going — next boot will retry.
                if let Err(e) = disable_for_health_failure(&pool, server_id).await {
                    tracing::error!(
                        server_id = %server_id,
                        error = ?e,
                        "mcp::health: failed to auto-disable server",
                    );
                }
                if let Err(e) = record_health_check_on(&pool, server_id, "unhealthy", Some(&failure.reason)).await {
                    tracing::warn!(error = ?e, server_id = %server_id, "mcp::health: failed to record unhealthy status (non-fatal)");
                }
            }
        }
    }
}

/// UPDATE one row's `enabled` to false. Direct SQL — the public
/// `update_*_mcp_server` paths require the full request shape and
/// run additional validation that's unnecessary for this internal
/// auto-disable.
async fn disable_for_health_failure(
    pool: &PgPool,
    server_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE mcp_servers SET enabled = false, updated_at = NOW() WHERE id = $1",
        server_id,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

/// Record a boot-probe health result against an explicit `pool` — the
/// pool-taking twin of `McpRepository::record_health_check` (which writes via
/// the global `Repos` pool). `run_startup_health_check` uses this so its list /
/// record / disable all go through the one pool it was handed.
async fn record_health_check_on(
    pool: &PgPool,
    server_id: Uuid,
    status: &str,
    reason: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query!(
        "UPDATE mcp_servers
         SET last_health_check_at = NOW(),
             last_health_check_status = $2,
             last_health_check_reason = $3
         WHERE id = $1",
        server_id,
        status,
        reason,
    )
    .execute(pool)
    .await
    .map_err(AppError::database_error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    /// Drives the REAL boot-time `run_startup_health_check` — the
    /// restart/resume-after-crash recovery path (audit all-19618bab49c1):
    /// on every server (re)start it re-probes each enabled, non-built-in
    /// MCP server and auto-disables the ones that no longer connect, so a
    /// process that crashed (or whose upstream MCP servers died while it was
    /// down) recovers a coherent health state at the next boot instead of
    /// leaving dead servers `enabled` in users' tool lists.
    ///
    /// This is the genuinely durable recovery seam: the in-memory pieces
    /// (`elicitation::registry` oneshot channels, live connection handles in
    /// `manager`) are reconstructed from scratch on reconnect and carry no
    /// cross-restart state — the boot health-check is what re-establishes the
    /// persisted MCP state, and it had no test.
    ///
    /// The test seeds an enabled HTTP server pointing at a closed loopback
    /// port (instant connection-refused, no network egress), simulates a boot
    /// by calling `run_startup_health_check`, and asserts the unreachable
    /// server was flipped to `enabled = false` with a persisted `unhealthy`
    /// health record — exactly the recovery a real restart performs.
    ///
    /// DB-gated soft-skip (mirrors the suite's env-gated real-stack tests) so
    /// `cargo test --lib` without Postgres stays green; runs for real wherever
    /// `DATABASE_URL` points at a migrated DB.
    #[tokio::test]
    async fn startup_health_check_auto_disables_unreachable_server_on_boot() {
        let url = match std::env::var("DATABASE_URL") {
            Ok(u) => u,
            Err(_) => {
                eprintln!("skip: DATABASE_URL unset — no DB to exercise run_startup_health_check");
                return;
            }
        };
        let pool = match PgPoolOptions::new().max_connections(2).connect(&url).await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("skip: DB unreachable ({e})");
                return;
            }
        };
        // `run_startup_health_check` reads + writes via the global `Repos`;
        // init is idempotent (no-op if another lib test already won the race).
        crate::core::init_repositories(pool.clone());

        // An enabled, non-built-in HTTP server whose URL is a closed loopback
        // port — the probe must fail (connection refused) exactly as it would
        // for a server whose upstream died while the process was down.
        let name = format!("crash-recovery-probe-{}", Uuid::new_v4());
        let server_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO mcp_servers (name, display_name, transport_type, url, enabled, is_system)
            VALUES ($1, $1, 'http', 'http://127.0.0.1:1/mcp', true, true)
            RETURNING id
            "#,
        )
        .bind(&name)
        .fetch_one(&pool)
        .await
        .expect("seed enabled unreachable mcp server");

        // Sanity: it really is enabled going into the "boot".
        let enabled_before: bool =
            sqlx::query_scalar("SELECT enabled FROM mcp_servers WHERE id = $1")
                .bind(server_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert!(enabled_before, "precondition: seeded server must be enabled");

        // Simulate the boot recovery pass.
        run_startup_health_check(pool.clone()).await;

        // Recovery outcome: the unreachable server was auto-disabled and its
        // failure was recorded — the persisted health state the next-boot
        // recovery is responsible for.
        let (enabled_after, status): (bool, Option<String>) = sqlx::query_as(
            "SELECT enabled, last_health_check_status FROM mcp_servers WHERE id = $1",
        )
        .bind(server_id)
        .fetch_one(&pool)
        .await
        .expect("re-read server after startup health check");

        assert!(
            !enabled_after,
            "run_startup_health_check must auto-disable an unreachable server on boot"
        );
        assert_eq!(
            status.as_deref(),
            Some("unhealthy"),
            "the boot probe failure must be recorded as an unhealthy health-check"
        );

        // Cleanup so repeated runs + sibling lib tests stay isolated.
        let _ = sqlx::query("DELETE FROM mcp_servers WHERE id = $1")
            .bind(server_id)
            .execute(&pool)
            .await;
    }
}

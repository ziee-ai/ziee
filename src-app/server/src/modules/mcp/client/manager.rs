use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::RwLock;
use uuid::Uuid;
use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::Value;

use super::session::McpSession;
use crate::common::AppError;
use crate::core::{config::Config, Repos};
use crate::modules::auth::jwt::Claims;
use crate::modules::mcp::models::McpServer;
use crate::modules::mcp::tool_calls::models::{McpCallContext, McpToolCallSource};

/// Process-wide handle to the session manager constructed in
/// `main.rs`. The event-handler path (`McpSessionCleanupHandler`)
/// needs to call `close(server_id)` when a server row is deleted —
/// but event handlers are registered via the `AppModule` trait which
/// runs BEFORE `main.rs` instantiates the session manager. The
/// Axum-Extension injection used by HTTP handlers can't reach them.
///
/// `main.rs` calls `set_global(...)` once at boot. Read via
/// `global()`; returns `None` in pre-init test scaffolding (unit
/// tests that don't go through `main.rs`).
static MCP_SESSION_MANAGER: OnceLock<Arc<McpSessionManager>> = OnceLock::new();

/// Install the process-wide session-manager handle. Idempotent on the
/// second call (subsequent `set` attempts are silently dropped — boot
/// only calls this once, but unit-test harnesses might call it from a
/// shared setup function).
#[allow(dead_code)]
pub fn set_global(manager: Arc<McpSessionManager>) {
    let _ = MCP_SESSION_MANAGER.set(manager);
}

/// Read the process-wide session-manager handle. None when called
/// before `set_global` (e.g. unit tests that don't boot `main.rs`).
pub fn global() -> Option<Arc<McpSessionManager>> {
    MCP_SESSION_MANAGER.get().cloned()
}

/// Idle reaper cadence: how often the background task scans the pool.
#[allow(dead_code)] // reached only from `spawn_idle_reaper`, wired in the bin (main.rs)
const REAPER_TICK: std::time::Duration = std::time::Duration::from_secs(60);

/// A pooled session untouched for longer than this is closed by the
/// reaper. Sessions are re-created lazily on the next `get_or_create`,
/// so eviction only costs a reconnect on the next use — worth it to
/// release the underlying subprocess / HTTP keep-alive of a server the
/// user has stopped chatting with. Mirrors `llm_local_runtime`'s
/// idle-unload, but MCP has no per-server admin setting so the
/// threshold is a compile-time constant.
#[allow(dead_code)] // reached only from `spawn_idle_reaper`, wired in the bin (main.rs)
const REAPER_MAX_IDLE_SECONDS: u64 = 30 * 60;

pub struct McpSessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<McpSession>>>>>,
    config: Arc<Config>,
}

impl McpSessionManager {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    #[allow(dead_code)]
    pub async fn get_or_create(
        &self,
        server_id: Uuid,
    ) -> Result<Arc<RwLock<McpSession>>, AppError> {
        // Check if session exists
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&server_id) {
                return Ok(session.clone());
            }
        }

        // Load server config from database
        let server = Repos.mcp.get_any_server(server_id).await?
            .ok_or_else(|| AppError::not_found("Server not found"))?;

        // Check if server is enabled
        if !server.enabled {
            return Err(AppError::bad_request("server_disabled", "Server is disabled"));
        }

        // Create new session
        let session = McpSession::new(server).await?;
        let session = Arc::new(RwLock::new(session));

        // Store session
        let mut sessions = self.sessions.write().await;
        sessions.insert(server_id, session.clone());

        Ok(session)
    }

    /// Get or create a session with conversation context headers injected.
    /// Always creates an EPHEMERAL (non-pooled) session — for both built-in
    /// servers (with X-Conversation-Id / X-Message-Id / a short-lived JWT) and
    /// regular servers (so parallel tool execution doesn't share one session).
    /// The ephemerality is what makes stamping `call_ctx` race-free: every
    /// tool call gets its own freshly-stamped session. `source` records how
    /// the call was triggered (chat / rest / always / approval / sampling).
    pub async fn get_or_create_with_context(
        &self,
        server_id: Uuid,
        user_id: Uuid,
        conversation_id: Option<Uuid>,
        branch_id: Option<Uuid>,
        message_id: Option<Uuid>,
        tool_use_id: Option<String>,
        source: McpToolCallSource,
    ) -> Result<Arc<RwLock<McpSession>>, AppError> {
        let server = Repos.mcp.get_any_server(server_id).await?
            .ok_or_else(|| AppError::not_found("Server not found"))?;

        if !server.enabled {
            return Err(AppError::bad_request("server_disabled", "Server is disabled"));
        }

        // Recording context stamped onto whichever session we build below.
        let call_ctx = McpCallContext {
            user_id: Some(user_id),
            conversation_id,
            branch_id,
            message_id,
            tool_use_id,
            source,
            server_name: server.name.clone(),
            is_built_in: server.is_built_in,
            // Stamped post-creation by the workflow dispatcher (set_workflow_run);
            // every other caller leaves it None.
            workflow_run_id: None,
            // Stamped post-creation by the agent dispatcher (set_review_classification).
            review_classification: None,
        };

        // For built-in servers: create ephemeral session with dynamic headers
        if server.is_built_in {
            let mut server_with_ctx = server.clone();
            self.inject_builtin_context_headers(
                &mut server_with_ctx,
                user_id,
                conversation_id,
                message_id,
            )
            .await?;

            // Ephemeral session — not stored in the pool
            let mut session = McpSession::new(server_with_ctx).await?;
            session.set_call_context(call_ctx);
            return Ok(Arc::new(RwLock::new(session)));
        }

        // Non-built-in: create ephemeral session per call (no pool, allows parallel tool execution)
        let mut session = McpSession::new(server).await?;
        session.set_call_context(call_ctx);
        Ok(Arc::new(RwLock::new(session)))
    }

    /// Re-fetch the UN-REDACTED server row for building an OUTBOUND session.
    ///
    /// `list_accessible` (repository.rs) nulls `url` for `is_system` servers so a
    /// regular user can't learn the admin-configured URL. That redacted view is
    /// correct for user-facing responses, but it must NEVER be used to build the
    /// server-side session/transport: `HttpMcpClient` then fails with
    /// `MISSING_URL` and sampling / always-mode silently break for system servers
    /// (a user server works only because its URL isn't redacted). The non-sampling
    /// execution path already avoids this by re-fetching via `get_any_server`
    /// inside `get_or_create_with_context`; the direct `new_with_sampling` /
    /// always-mode builds must do the same. Returns the full row with the real URL.
    ///
    /// Unlike `get_or_create_with_context`, this does NOT re-check `server.enabled`
    /// and does NOT inject built-in context headers: callers pass a `server.id` that
    /// was already resolved from the caller's accessible-server set (which is
    /// enabled-filtered upstream in `get_all_accessible_config`), and the sampling /
    /// always-mode direct-build path is for external `supports_sampling` servers, not
    /// loopback built-ins. Keep it a thin un-redacted re-fetch.
    pub async fn resolve_server_for_session(
        &self,
        server_id: Uuid,
    ) -> Result<McpServer, AppError> {
        Repos
            .mcp
            .get_any_server(server_id)
            .await?
            .ok_or_else(|| AppError::not_found("Server not found"))
    }

    /// Inject the loopback auth + context headers a **built-in** server needs
    /// onto `server.headers`: a short-lived per-user JWT (satisfying the
    /// built-in route's `RequirePermissions` gate) plus optional
    /// `X-Conversation-Id` / `X-Message-Id` context.
    ///
    /// This is the SINGLE place a built-in server is authenticated. Both the
    /// live session path (`get_or_create_with_context`) AND the connection-test
    /// probe (`handlers::test_connection`) call it, so ANY built-in server —
    /// including ones added in the future — authenticates identically and
    /// passes its "Test connection" with no extra per-server wiring. Do not
    /// re-implement the JWT minting elsewhere; route new built-in call sites
    /// through this helper.
    ///
    /// TTL is 60s (not 5s): a built-in tool call can chain multiple hops (e.g.
    /// control's `invoke_capability` re-dispatches to a REST route over
    /// loopback, forwarding this same token) and, under a slow model or loaded
    /// host, a 5s window could expire mid-chain → spurious 401s. 60s stays
    /// short-lived (loopback-only, per-user) with headroom for multi-hop.
    ///
    /// Async because the minted token must carry the user's CURRENT
    /// access-token revocation epoch — see `generate_short_lived_jwt`.
    pub async fn inject_builtin_context_headers(
        &self,
        server: &mut McpServer,
        user_id: Uuid,
        conversation_id: Option<Uuid>,
        message_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        let mut headers = server.headers.as_object().cloned().unwrap_or_default();

        if let Some(cid) = conversation_id {
            headers.insert("x-conversation-id".to_string(), Value::String(cid.to_string()));
        }
        if let Some(msg_id) = message_id {
            headers.insert("x-message-id".to_string(), Value::String(msg_id.to_string()));
        }

        // Only mint if the row didn't already carry an Authorization header.
        if !headers.contains_key("authorization") && !headers.contains_key("Authorization") {
            let token = Self::generate_short_lived_jwt(
                user_id,
                &self.config.jwt.secret,
                &self.config.jwt.issuer,
                &self.config.jwt.audience,
                60,
                crate::modules::auth::refresh_tokens::current_token_version(Repos.pool(), user_id)
                    .await?
                    .ok_or_else(|| AppError::unauthorized("USER_NOT_FOUND", "User not found"))?,
            )?;
            headers.insert(
                "Authorization".to_string(),
                Value::String(format!("Bearer {}", token)),
            );
        }

        server.headers = Value::Object(headers);
        Ok(())
    }

    /// The deployment JWT secret. Used by the workflow `ToolDispatcher` (E9) so
    /// it can pass a secret to `resource_link::persist_links` — letting a tool's
    /// token-based `http://` loopback resource_links be fetched + persisted, not
    /// just in-process `ziee://` host-path links.
    pub fn jwt_secret(&self) -> &str {
        &self.config.jwt.secret
    }

    /// The deployment JWT issuer/audience — MUST accompany `jwt_secret()` when
    /// minting an internal token (see `generate_short_lived_jwt`).
    pub fn jwt_issuer(&self) -> &str {
        &self.config.jwt.issuer
    }

    pub fn jwt_audience(&self) -> &str {
        &self.config.jwt.audience
    }

    /// Generate a short-lived JWT for internal service-to-service calls.
    ///
    /// `issuer`/`audience` MUST come from the deployment config — hardcoding
    /// `"ziee"`/`"ziee-api"` breaks token validation on any deployment (or test)
    /// whose `jwt.issuer`/`jwt.audience` differs (the validator rejects with
    /// `InvalidIssuer`), which silently 401s every built-in MCP server.
    ///
    /// `token_version` MUST be the user's CURRENT `users.token_version` (read it
    /// with `auth::refresh_tokens::current_token_version`), NOT a constant. This
    /// token is validated by the same `RequirePermissions` gate as any user
    /// token, so a stale/defaulted epoch would 401 every built-in MCP call for
    /// any user who has ever logged out. It is safe to stamp the CURRENT epoch:
    /// this token is minted server-side, seconds before use, on behalf of an
    /// already-authenticated request — it is not a credential the user holds,
    /// and its 10-60s TTL bounds it far below the epoch's purpose (killing
    /// long-lived tokens that outlive a logout).
    pub fn generate_short_lived_jwt(
        user_id: Uuid,
        secret: &str,
        issuer: &str,
        audience: &str,
        ttl_seconds: i64,
        token_version: i32,
    ) -> Result<String, AppError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(ttl_seconds);
        let claims = Claims {
            sub: user_id.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
            iss: issuer.to_string(),
            aud: audience.to_string(),
            username: String::new(),
            email: String::new(),
            is_admin: false,
            jti: None,
            ver: Some(token_version),
        };
        encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(secret.as_bytes()),
        )
        .map_err(|e| AppError::internal_error(format!("Failed to generate internal JWT: {}", e)))
    }

    pub async fn close(&self, server_id: Uuid) -> Result<(), AppError> {
        let session = {
            let mut sessions = self.sessions.write().await;
            sessions.remove(&server_id)
        };

        if let Some(session) = session {
            let mut session = session.write().await;
            session.disconnect().await?;
        }

        Ok(())
    }

    #[allow(dead_code)] // Used in main.rs for graceful shutdown (binary only)
    pub async fn close_all(&self) -> Result<(), AppError> {
        let sessions = {
            let mut sessions = self.sessions.write().await;
            
            sessions.drain().collect::<Vec<_>>()
        };

        for (_, session) in sessions {
            let mut session = session.write().await;
            let _ = session.disconnect().await;
        }

        Ok(())
    }

    /// Whether a session for `server_id` is currently pooled. Drives
    /// the cleanup test that asserts `McpSessionCleanupHandler` actually
    /// removed an entry from the pool after a delete event.
    #[allow(dead_code)]
    pub async fn contains(&self, server_id: Uuid) -> bool {
        self.sessions.read().await.contains_key(&server_id)
    }

    /// Spawn the background idle-session reaper. Ticks every
    /// [`REAPER_TICK`] and closes any pooled session idle longer than
    /// [`REAPER_MAX_IDLE_SECONDS`]. Called once from `main.rs` after the
    /// manager is installed as the process-wide handle. Returns the
    /// `JoinHandle` (mirrors `llm_local_runtime::reaper::spawn`); boot
    /// drops it — the task lives for the process lifetime.
    #[allow(dead_code)] // called from the bin (main.rs); the lib compiles standalone
    pub fn spawn_idle_reaper(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let manager = self.clone();
        tokio::spawn(async move {
            tracing::info!(
                "mcp::session reaper: started (tick {}s, max_idle {}s)",
                REAPER_TICK.as_secs(),
                REAPER_MAX_IDLE_SECONDS
            );
            let mut interval = tokio::time::interval(REAPER_TICK);
            // Skip the immediate first tick (interval fires once at t=0).
            interval.tick().await;
            loop {
                interval.tick().await;
                match manager.cleanup_idle(REAPER_MAX_IDLE_SECONDS).await {
                    Ok(n) if n > 0 => {
                        tracing::debug!("mcp::session reaper: closed {} idle session(s)", n);
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::warn!("mcp::session reaper tick failed: {}", e);
                    }
                }
            }
        })
    }

    #[allow(dead_code)] // driven by `spawn_idle_reaper`, wired in the bin (main.rs)
    pub async fn cleanup_idle(&self, max_idle_seconds: u64) -> Result<usize, AppError> {
        let to_remove = {
            let sessions = self.sessions.read().await;
            let mut to_remove = Vec::new();

            for (server_id, session) in sessions.iter() {
                let session = session.read().await;
                if session.idle_time().as_secs() > max_idle_seconds {
                    to_remove.push(*server_id);
                }
            }

            to_remove
        };

        for server_id in &to_remove {
            self.close(*server_id).await?;
        }

        Ok(to_remove.len())
    }
}

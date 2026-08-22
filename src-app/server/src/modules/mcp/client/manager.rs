use chrono::{Duration, Utc};
use jsonwebtoken::{EncodingKey, Header, encode};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::Instant;
use tokio::sync::RwLock;
use uuid::Uuid;

use super::session::McpSession;
use crate::common::AppError;
use crate::core::{Repos, config::Config};
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

// ---------------------------------------------------------------------------
// Connection circuit-breaker
//
// A `get_or_create*` miss builds a fresh `McpSession`, whose `client.connect()`
// performs a TCP/stdio dial. For an UNREACHABLE server (process down, connect
// refused, host gone) that dial fails only AFTER blocking out its full
// connect_timeout — and with no failure state the very next call (and every
// turn thereafter) re-dials the same dead server. A corpus sweep observed
// ~30k such repeated re-dials, each paying the connect_timeout in per-turn
// latency.
//
// The breaker records per-server connection failures and short-circuits a
// re-dial while the server is inside an exponentially-growing cooldown window,
// so a down server costs one connect attempt per backoff window instead of one
// per turn. A SUCCESSFUL connect clears the breaker immediately, so a recovered
// server serves on the very next call with no lingering penalty.
// ---------------------------------------------------------------------------

/// First cooldown after a single connect failure. Deliberately small so a
/// briefly-flapping server recovers quickly; it doubles on each consecutive
/// failure up to [`BREAKER_MAX_COOLDOWN`].
const BREAKER_BASE_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(1);

/// Ceiling on the cooldown regardless of how many consecutive failures have
/// accrued. Five minutes bounds a long-down server to ~12 retries/hour (rather
/// than one per turn) while still letting a recovered server be re-tried within
/// minutes of coming back — the same order of magnitude as the idle reaper.
const BREAKER_MAX_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Per-server connect-failure state driving the circuit-breaker. An entry
/// exists ONLY while a server is failing; a successful connect removes it.
#[derive(Clone)]
struct BreakerState {
    /// Instant of the most recent failed connect attempt.
    last_failure: Instant,
    /// Number of consecutive failures — drives the backoff exponent. Always
    /// `>= 1` for a live entry (incremented on each failure, entry removed on
    /// success).
    consecutive: u32,
    /// The most recent connection error message, replayed while the breaker is
    /// open so the caller sees the real cause instead of a generic message.
    last_error: String,
}

/// Cooldown window for a server with `consecutive` consecutive failures:
/// `BREAKER_BASE_COOLDOWN * 2^(consecutive - 1)`, saturating at
/// [`BREAKER_MAX_COOLDOWN`]. `consecutive <= 1` yields the base cooldown; the
/// shift is bounded so a large failure count can never overflow.
fn breaker_backoff(consecutive: u32) -> std::time::Duration {
    let base_secs = BREAKER_BASE_COOLDOWN.as_secs().max(1);
    let max_secs = BREAKER_MAX_COOLDOWN.as_secs();
    let shift = consecutive.saturating_sub(1).min(63);
    let scaled = base_secs.checked_shl(shift).unwrap_or(u64::MAX);
    std::time::Duration::from_secs(scaled.min(max_secs))
}

/// Whether a fresh connect should be attempted now, given a server's breaker
/// state. `None` (never failed, or cleared by a prior success) → always
/// attempt. Otherwise attempt only once the cooldown window for the recorded
/// consecutive-failure count has fully elapsed since the last failure.
fn should_attempt_connect(state: Option<&BreakerState>, now: Instant) -> bool {
    match state {
        None => true,
        Some(s) => now.saturating_duration_since(s.last_failure) >= breaker_backoff(s.consecutive),
    }
}

pub struct McpSessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, Arc<RwLock<McpSession>>>>>,
    /// Per-server connection circuit-breaker state. Keyed by `server_id`; an
    /// entry is present only while a server is in a failing streak (removed on
    /// the first successful connect). See the module-level breaker comment.
    failures: Arc<RwLock<HashMap<Uuid, BreakerState>>>,
    config: Arc<Config>,
}

impl McpSessionManager {
    pub fn new(config: Arc<Config>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            failures: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Circuit-breaker gate: if `server_id` is inside its cooldown window,
    /// return the cached connection error IMMEDIATELY (no fresh dial). Returns
    /// `Ok(())` when a connect should be attempted (no state, or the window has
    /// elapsed). Called on every `get_or_create*` MISS, before building a
    /// session — see the module-level breaker comment.
    async fn check_connection_breaker(&self, server_id: Uuid) -> Result<(), AppError> {
        let failures = self.failures.read().await;
        if let Some(state) = failures.get(&server_id)
            && !should_attempt_connect(Some(state), Instant::now())
        {
            return Err(AppError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "MCP_SERVER_UNREACHABLE",
                format!(
                    "MCP server is unreachable and in a connection cooldown after \
                     {} consecutive failure(s); last error: {}",
                    state.consecutive, state.last_error
                ),
            ));
        }
        Ok(())
    }

    /// Record a failed connect for `server_id`: create or increment its breaker
    /// entry, stamp the failure time, and cache the error for replay while the
    /// breaker is open.
    async fn record_connection_failure(&self, server_id: Uuid, err: &AppError) {
        let mut failures = self.failures.write().await;
        let entry = failures.entry(server_id).or_insert_with(|| BreakerState {
            last_failure: Instant::now(),
            consecutive: 0,
            last_error: String::new(),
        });
        entry.last_failure = Instant::now();
        entry.consecutive = entry.consecutive.saturating_add(1);
        entry.last_error = err.to_string();
    }

    /// Clear any breaker entry for `server_id` after a successful connect, so a
    /// recovered server serves immediately on the next call.
    async fn clear_connection_breaker(&self, server_id: Uuid) {
        self.failures.write().await.remove(&server_id);
    }

    /// Build an `McpSession` (which dials on connect) while maintaining the
    /// connection circuit-breaker: clear the breaker on success, record/increment
    /// it on failure. The caller MUST have already passed `check_connection_breaker`.
    async fn create_session_tracked(
        &self,
        server_id: Uuid,
        server: McpServer,
    ) -> Result<McpSession, AppError> {
        match McpSession::new(server).await {
            Ok(session) => {
                self.clear_connection_breaker(server_id).await;
                Ok(session)
            }
            Err(e) => {
                self.record_connection_failure(server_id, &e).await;
                Err(e)
            }
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
        let server = Repos
            .mcp
            .get_any_server(server_id)
            .await?
            .ok_or_else(|| AppError::not_found("Server not found"))?;

        // Check if server is enabled
        if !server.enabled {
            return Err(AppError::bad_request(
                "server_disabled",
                "Server is disabled",
            ));
        }

        // Circuit-breaker: don't re-dial a server still inside its cooldown.
        self.check_connection_breaker(server_id).await?;

        // Create new session (this performs the connect); breaker is updated inside.
        let session = self.create_session_tracked(server_id, server).await?;
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
        let server = Repos
            .mcp
            .get_any_server(server_id)
            .await?
            .ok_or_else(|| AppError::not_found("Server not found"))?;

        if !server.enabled {
            return Err(AppError::bad_request(
                "server_disabled",
                "Server is disabled",
            ));
        }

        // Circuit-breaker: don't re-dial a server still inside its cooldown.
        self.check_connection_breaker(server_id).await?;

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
            let mut session = self
                .create_session_tracked(server_id, server_with_ctx)
                .await?;
            session.set_call_context(call_ctx);
            return Ok(Arc::new(RwLock::new(session)));
        }

        // Non-built-in: create ephemeral session per call (no pool, allows parallel tool execution)
        let mut session = self.create_session_tracked(server_id, server).await?;
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
    pub async fn resolve_server_for_session(&self, server_id: Uuid) -> Result<McpServer, AppError> {
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
            headers.insert(
                "x-conversation-id".to_string(),
                Value::String(cid.to_string()),
            );
        }
        if let Some(msg_id) = message_id {
            headers.insert(
                "x-message-id".to_string(),
                Value::String(msg_id.to_string()),
            );
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

#[cfg(test)]
mod breaker_tests {
    use super::*;
    use std::time::Duration as StdDuration;

    /// A breaker entry whose last failure was `since` ago, with `consecutive`
    /// consecutive failures recorded.
    fn state(consecutive: u32, since: StdDuration) -> BreakerState {
        BreakerState {
            last_failure: Instant::now()
                .checked_sub(since)
                .unwrap_or_else(Instant::now),
            consecutive,
            last_error: "connect refused".to_string(),
        }
    }

    #[test]
    fn no_state_always_attempts() {
        assert!(should_attempt_connect(None, Instant::now()));
    }

    #[test]
    fn cooldown_active_suppresses_redial() {
        // One failure just now → inside the base cooldown → must NOT re-dial.
        let s = state(1, StdDuration::from_millis(0));
        assert!(
            !should_attempt_connect(Some(&s), Instant::now()),
            "a server inside its cooldown window must not be re-dialed"
        );
    }

    #[test]
    fn window_elapsed_allows_retry() {
        // One failure, but longer ago than backoff(1) (=1s) → retry allowed.
        let s = state(1, StdDuration::from_secs(2));
        assert!(should_attempt_connect(Some(&s), Instant::now()));
    }

    #[test]
    fn success_clears_then_attempts() {
        // A deep failing streak is suppressed …
        let failing = state(5, StdDuration::from_millis(0));
        assert!(
            !should_attempt_connect(Some(&failing), Instant::now()),
            "a long failing streak stays suppressed inside its (longer) cooldown"
        );
        // … but once a success clears the entry (None), the next call attempts.
        assert!(
            should_attempt_connect(None, Instant::now()),
            "a cleared breaker (success) must attempt on the next call"
        );
    }

    #[test]
    fn backoff_is_exponential() {
        assert_eq!(breaker_backoff(1), BREAKER_BASE_COOLDOWN);
        assert_eq!(breaker_backoff(2), BREAKER_BASE_COOLDOWN * 2);
        assert_eq!(breaker_backoff(3), BREAKER_BASE_COOLDOWN * 4);
        assert_eq!(breaker_backoff(4), BREAKER_BASE_COOLDOWN * 8);
    }

    #[test]
    fn backoff_saturates_at_cap() {
        assert_eq!(breaker_backoff(1_000_000), BREAKER_MAX_COOLDOWN);
        // No count, however large, may exceed the cap or overflow.
        for c in [10u32, 20, 31, 32, 63, 64, 100, u32::MAX] {
            assert!(
                breaker_backoff(c) <= BREAKER_MAX_COOLDOWN,
                "backoff({c}) exceeded the cap"
            );
        }
    }

    #[test]
    fn backoff_zero_is_base() {
        // `consecutive` is >= 1 for a live entry, but the shift math must not
        // underflow for 0 — it saturates to the base cooldown.
        assert_eq!(breaker_backoff(0), BREAKER_BASE_COOLDOWN);
    }
}

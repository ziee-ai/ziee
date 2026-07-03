//! Same-port reverse proxy for local LLM engines.
//!
//! The chat module treats a local provider as just another
//! OpenAI-compatible endpoint: it does
//! `Provider::new("local", PROXY_TOKEN, base_url)` where
//! `base_url` is the URL this module mounts. The proxy is the
//! single auth + routing + auto-start + drain boundary; chat code
//! never branches on `"local"`.
//!
//! ## Design notes
//!
//! - `LOCAL_PROXY_PATH` is the single source of truth for both the
//!   axum route mount AND the URL handed to chat code via the
//!   repository's read-time injection (see `repository::resolve_runtime_fields`).
//!   Bump it here and every caller picks up the new value — zero DB
//!   migrations needed.
//! - The `PROXY_TOKEN_CACHE` is populated at module init from
//!   `SELECT id, api_key FROM llm_providers WHERE provider_type = 'local'`
//!   and invalidated on provider create / rotate / delete. Hot-path
//!   lookup is O(1) hash + constant-time array compare; no per-request
//!   DB decrypt.
//! - In-memory drain coordination: `INSTANCE_DRAINING` flips a flag
//!   the proxy front door observes (returns 503), `ProcessInfo.in_flight`
//!   gets incremented per forwarded request via a `Drop`-guarded
//!   counter so panic / disconnect still decrements.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};

use sqlx::types::Uuid;
use sqlx::PgPool;
use tokio::sync::RwLock;

use crate::common::AppError;

/// Single source of truth for the proxy path, RELATIVE to the
/// server's `api_prefix`. Module routers are nested under `api_prefix`
/// in `app_builder.rs`, so this is mounted bare (no leading `/api`).
/// Change here and:
///  - `proxy_router` re-mounts under the new prefix
///  - `derive_proxy_url` hands out the new (api_prefix-prepended) URL
///  - chat module's outbound URL updates on next read (no migration)
pub const LOCAL_PROXY_PATH: &str = "/local-llm/v1";

/// Derive the live proxy URL from the server's listen config + the
/// configured `api_prefix`. Module routes are nested under
/// `api_prefix` (`app_builder.rs`), so the externally-reachable URL is
/// `http://host:port{api_prefix}{LOCAL_PROXY_PATH}`.
///
/// For `0.0.0.0` bind we force `127.0.0.1` in the URL — chat code's
/// outbound call is in-process, and we never want a public-facing
/// `0.0.0.0` URL to leak into admin UI / audit logs.
pub fn derive_proxy_url(server_host: &str, server_port: u16, api_prefix: &str) -> String {
    let host = if server_host == "0.0.0.0" {
        "127.0.0.1"
    } else {
        server_host
    };
    // Normalize: api_prefix is like "/api" (leading slash, no trailing).
    let prefix = api_prefix.trim_end_matches('/');
    format!("http://{host}:{server_port}{prefix}{LOCAL_PROXY_PATH}")
}

// =====================================================================
// PROXY_TOKEN_CACHE — in-memory hash → provider_id lookup
// =====================================================================

/// SHA-256 of the bearer token (32 bytes). Used as the cache key so
/// we never hold the plaintext in memory beyond `insert`.
pub type TokenHash = [u8; 32];

/// Lazy global cache. The shape is read-mostly (validation per request,
/// writes only on create / rotate / delete) so an RwLock is right —
/// reads stay cheap.
static PROXY_TOKEN_CACHE: LazyLock<RwLock<HashMap<TokenHash, Uuid>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

/// Hash a plaintext token with sha256 → 32 bytes.
pub fn hash_token(token: &str) -> TokenHash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let result = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&result);
    out
}

/// Insert a token hash → provider_id mapping. Called by the
/// llm_provider module's create / rotate handlers.
pub async fn insert_token(token: &str, provider_id: Uuid) {
    let hash = hash_token(token);
    PROXY_TOKEN_CACHE.write().await.insert(hash, provider_id);
}

/// Remove a token hash. Called on provider delete or token rotation
/// (after the new token's hash is already inserted to avoid a
/// brief window where neither matches).
pub async fn remove_token(token: &str) {
    let hash = hash_token(token);
    PROXY_TOKEN_CACHE.write().await.remove(&hash);
}

/// Wipe the cache. Called only by tests + boot-time reseeding.
#[allow(dead_code)] // test-only + boot-reseed caller; invisible to lib dead-code analysis
pub async fn clear_cache() {
    PROXY_TOKEN_CACHE.write().await.clear();
}

/// Constant-time lookup of a presented bearer token against the cache.
/// Returns the matching provider_id on success.
///
/// Uses `subtle::ConstantTimeEq` on the hash arrays so a wrong token
/// can't leak a per-byte timing differential against valid tokens.
/// (The hash is already a one-way function — the constant-time
/// comparison is belt + suspenders.)
pub async fn lookup_token(presented: &str) -> Option<Uuid> {
    let presented_hash = hash_token(presented);
    let cache = PROXY_TOKEN_CACHE.read().await;
    // Linear scan with constant-time per-entry compare. The cache is
    // tiny (one local provider in the common case, a handful at most)
    // so O(n) is correct; the constant-time compare matters for the
    // single-entry hot path.
    for (stored_hash, provider_id) in cache.iter() {
        if constant_time_eq(stored_hash, &presented_hash) {
            return Some(*provider_id);
        }
    }
    None
}

/// Constant-time array equality. No standard-library equivalent
/// without pulling in `subtle`.
fn constant_time_eq(a: &TokenHash, b: &TokenHash) -> bool {
    debug_assert_eq!(a.len(), b.len());
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

// =====================================================================
// Boot-time reseed
// =====================================================================

/// Read every local provider's api_key (decrypting at-rest where
/// configured), and populate the cache. Called once at module init
/// AFTER the `llm_provider` module is up.
///
/// Best-effort per provider: a single provider that fails to resolve or mint
/// is logged and skipped — it does NOT abort the reseed for the others. The
/// cache is cleared + repopulated atomically at the END (one write-lock
/// critical section, no awaits held), so `lookup_token` never observes a
/// transiently-empty cache and a concurrent create/rotate `insert_token` isn't
/// clobbered by an up-front clear.
///
/// Decryption mirrors `llm_provider::repositories::admin`: fetch the
/// encrypted bytea + plaintext columns and resolve in Rust via
/// `common::secret::resolve_optional_secret` (which reads the storage
/// key from `core::secrets`), rather than `pgp_sym_decrypt` inline in
/// SQL. Keeps the query macro-checkable + consistent with the rest of
/// the codebase.
pub async fn reseed_from_db(pool: &PgPool) -> Result<(), AppError> {
    let rows = sqlx::query!(
        "SELECT id, api_key, api_key_encrypted FROM llm_providers
         WHERE provider_type = 'local'",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::internal_error(format!("token reseed failed: {e}")))?;

    // Resolve (and, for keyless local providers, MINT + PERSIST) each token
    // first, then populate the cache in one shot. We deliberately do NOT hold
    // the cache write-lock across the awaited DB work below.
    let mut to_insert: Vec<(TokenHash, Uuid)> = Vec::with_capacity(rows.len());
    let mut minted = 0usize;
    for r in rows {
        let resolved =
            crate::common::secret::resolve_optional_secret(pool, r.api_key_encrypted, r.api_key)
                .await;

        // A local provider with no usable token (NULL or blank) — notably the
        // built-in seeded `Local` provider, which is inserted with a NULL
        // api_key and never goes through the auto-mint path in
        // `create_provider` — cannot authenticate proxy requests. Mint a token,
        // persist it through the repository so the dual-column at-rest
        // encryption matches create/rotate (and NO event is emitted — reseed is
        // an internal boot step), then cache it. Idempotent across reboots: once
        // persisted, the next boot resolves a non-empty key and takes the fast
        // path below without re-minting.
        let token = match resolved {
            Some(t) if !t.trim().is_empty() => t,
            _ => {
                let token = generate_proxy_token();
                // Persist through the same pool reseed was handed (NOT the global
                // `Repos`): reseed runs both at boot and from integration tests
                // that drive it directly with their own pool. The free
                // `update_llm_provider` reuses the dual-column at-rest encryption
                // from create/rotate and emits no event. Best-effort: warn + skip
                // this provider on failure (or if its row vanished mid-reseed) so
                // one bad row can't poison the whole cache.
                match crate::modules::llm_provider::repositories::admin::update_llm_provider(
                    pool,
                    r.id,
                    crate::modules::llm_provider::types::UpdateLlmProviderRequest {
                        api_key: Some(token.clone()),
                        name: None,
                        enabled: None,
                        base_url: None,
                        proxy_settings: None,
                    },
                )
                .await
                {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        tracing::warn!(
                            "llm_local_runtime: local provider {} vanished mid-reseed; skipping",
                            r.id
                        );
                        continue;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "llm_local_runtime: minting proxy token for local provider {} failed: \
                             {e}; skipping (its proxy requests will 401 until the next reseed)",
                            r.id
                        );
                        continue;
                    }
                }
                tracing::info!(
                    "llm_local_runtime: minted proxy token for keyless local provider {}",
                    r.id
                );
                minted += 1;
                token
            }
        };
        to_insert.push((hash_token(&token), r.id));
    }

    let count = to_insert.len();
    // Clear + repopulate atomically: `lookup_token` never observes an empty
    // cache, and no await is held inside the critical section.
    {
        let mut cache = PROXY_TOKEN_CACHE.write().await;
        cache.clear();
        for (hash, id) in to_insert {
            cache.insert(hash, id);
        }
    }

    tracing::info!(
        "llm_local_runtime: PROXY_TOKEN_CACHE reseeded with {} local provider(s) ({} newly minted)",
        count,
        minted
    );
    Ok(())
}

// =====================================================================
// Cryptographically secure token generation
// =====================================================================

/// Generate a 32-byte URL-safe token. Uses the OS RNG.
pub fn generate_proxy_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    // URL-safe base64 without padding so it can ship as-is in
    // `Authorization: Bearer <token>` and survive copy-paste.
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

// =====================================================================
// In-memory drain coordination
// =====================================================================

/// State of an instance from the PROXY's perspective. Lives in memory;
/// the reaper sets `Draining` before issuing `stop`, the proxy front
/// door sees `Draining` and returns 503.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstanceFlag {
    Active,
    Draining,
}

static INSTANCE_FLAGS: LazyLock<RwLock<HashMap<Uuid, InstanceFlag>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn set_instance_flag(model_id: Uuid, flag: InstanceFlag) {
    INSTANCE_FLAGS.write().await.insert(model_id, flag);
}

pub async fn get_instance_flag(model_id: Uuid) -> InstanceFlag {
    INSTANCE_FLAGS
        .read()
        .await
        .get(&model_id)
        .copied()
        .unwrap_or(InstanceFlag::Active)
}

pub async fn clear_instance_flag(model_id: Uuid) {
    INSTANCE_FLAGS.write().await.remove(&model_id);
}

// =====================================================================
// last_used_at debounced writer
// =====================================================================

/// Latest in-memory `last_used_at` per model. Written to on every
/// successful proxy forward. The debounced background writer
/// (started by the module) flushes to the DB every ~5s, so the hot
/// path doesn't pay a DB write per request.
static INSTANCE_LAST_USED: LazyLock<RwLock<HashMap<Uuid, chrono::DateTime<chrono::Utc>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

pub async fn touch_last_used(model_id: Uuid) {
    let now = chrono::Utc::now();
    INSTANCE_LAST_USED.write().await.insert(model_id, now);
}

/// Drain the in-memory `last_used_at` map and return the contents.
/// Used by the background writer to flush to PostgreSQL.
pub async fn drain_last_used() -> Vec<(Uuid, chrono::DateTime<chrono::Utc>)> {
    let mut map = INSTANCE_LAST_USED.write().await;
    map.drain().collect()
}

// =====================================================================
// In-flight request counter — drain coordination
// =====================================================================

use std::sync::atomic::{AtomicUsize, Ordering};

static INSTANCE_INFLIGHT: LazyLock<RwLock<HashMap<Uuid, Arc<AtomicUsize>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));

async fn get_or_create_inflight(model_id: Uuid) -> Arc<AtomicUsize> {
    {
        let read = INSTANCE_INFLIGHT.read().await;
        if let Some(c) = read.get(&model_id) {
            return c.clone();
        }
    }
    let mut write = INSTANCE_INFLIGHT.write().await;
    write
        .entry(model_id)
        .or_insert_with(|| Arc::new(AtomicUsize::new(0)))
        .clone()
}

/// RAII guard that increments the in-flight counter on construction
/// and decrements on drop. Safe under panic / early return /
/// client disconnect.
pub struct InFlightGuard {
    counter: Arc<AtomicUsize>,
}

impl InFlightGuard {
    pub async fn acquire(model_id: Uuid) -> Self {
        let counter = get_or_create_inflight(model_id).await;
        counter.fetch_add(1, Ordering::SeqCst);
        Self { counter }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Current in-flight count for an instance. Used by the reaper to
/// know when a drain has completed.
pub async fn inflight_count(model_id: Uuid) -> usize {
    let read = INSTANCE_INFLIGHT.read().await;
    read.get(&model_id)
        .map(|c| c.load(Ordering::SeqCst))
        .unwrap_or(0)
}

/// Remove the in-flight counter for a model.
///
/// DANGER: only safe when no `InFlightGuard` for this model can be
/// live. Removing the Arc while guards hold clones orphans the
/// counter — a new request then creates a fresh counter at 0, so
/// `inflight_count` under-reports and the reaper can SIGTERM an
/// engine with live streams (H1/H2). The hot-path callers were
/// removed; this remains only for test isolation, where the runtime
/// is single-threaded and no guards are live.
#[cfg(test)]
pub async fn forget_inflight(model_id: Uuid) {
    INSTANCE_INFLIGHT.write().await.remove(&model_id);
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn token_roundtrip_via_cache() {
        clear_cache().await;
        let provider_id = Uuid::new_v4();
        let token = generate_proxy_token();
        insert_token(&token, provider_id).await;

        let looked_up = lookup_token(&token).await;
        assert_eq!(looked_up, Some(provider_id));

        // Wrong token never resolves.
        assert_eq!(lookup_token("definitely-wrong").await, None);

        // Hash collisions are astronomically unlikely; a random
        // distinct string must not look up the inserted provider.
        assert_eq!(lookup_token(&generate_proxy_token()).await, None);
    }

    #[tokio::test]
    async fn token_remove_invalidates() {
        clear_cache().await;
        let pid = Uuid::new_v4();
        let token = generate_proxy_token();
        insert_token(&token, pid).await;
        assert_eq!(lookup_token(&token).await, Some(pid));
        remove_token(&token).await;
        assert_eq!(lookup_token(&token).await, None);
    }

    #[tokio::test]
    async fn token_constant_time_eq_self_consistent() {
        let a = hash_token("hello");
        let b = hash_token("hello");
        assert!(constant_time_eq(&a, &b));
        let c = hash_token("hello!"); // single-char diff
        assert!(!constant_time_eq(&a, &c));
    }

    #[tokio::test]
    async fn in_flight_guard_increments_and_decrements() {
        let model_id = Uuid::new_v4();
        forget_inflight(model_id).await;
        assert_eq!(inflight_count(model_id).await, 0);
        {
            let _g1 = InFlightGuard::acquire(model_id).await;
            assert_eq!(inflight_count(model_id).await, 1);
            let _g2 = InFlightGuard::acquire(model_id).await;
            assert_eq!(inflight_count(model_id).await, 2);
        }
        assert_eq!(inflight_count(model_id).await, 0);
    }

    #[tokio::test]
    async fn instance_flag_default_is_active() {
        let model_id = Uuid::new_v4();
        clear_instance_flag(model_id).await;
        assert_eq!(get_instance_flag(model_id).await, InstanceFlag::Active);
        set_instance_flag(model_id, InstanceFlag::Draining).await;
        assert_eq!(get_instance_flag(model_id).await, InstanceFlag::Draining);
        clear_instance_flag(model_id).await;
        assert_eq!(get_instance_flag(model_id).await, InstanceFlag::Active);
    }

    #[test]
    fn proxy_url_forces_loopback_on_0_0_0_0() {
        assert_eq!(
            derive_proxy_url("0.0.0.0", 3000, "/api"),
            "http://127.0.0.1:3000/api/local-llm/v1"
        );
        assert_eq!(
            derive_proxy_url("127.0.0.1", 3000, "/api"),
            "http://127.0.0.1:3000/api/local-llm/v1"
        );
        assert_eq!(
            derive_proxy_url("localhost", 8080, "/api"),
            "http://localhost:8080/api/local-llm/v1"
        );
        // Trailing slash on api_prefix is normalized away.
        assert_eq!(
            derive_proxy_url("127.0.0.1", 3000, "/api/"),
            "http://127.0.0.1:3000/api/local-llm/v1"
        );
    }

    #[test]
    fn generate_proxy_token_is_url_safe() {
        let t = generate_proxy_token();
        // 32 bytes base64 url-safe no-pad = 43 chars.
        assert_eq!(t.len(), 43);
        // No padding chars; only URL-safe alphabet.
        for c in t.chars() {
            assert!(
                c.is_ascii_alphanumeric() || c == '-' || c == '_',
                "non-url-safe char: {c}"
            );
        }
    }
}

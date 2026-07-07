//! Per-session bridge token store (ITEM-5).
//!
//! DEC-6: the bridge stamps a fresh per-session token into the served
//! `taskpane.html` at request time; the task pane then presents it back when it
//! opens the WSS connection (as a `Sec-WebSocket-Protocol` value) and on the
//! token-guarded POST sinks. The bridge rejects any WSS/POST without a valid
//! token (or with a disallowed Origin).
//!
//! This mirrors `llm_local_runtime/proxy.rs` in spirit — a 32-byte OS-RNG token
//! rendered base64url, of which we cache only the SHA-256 hash (never the
//! plaintext beyond the mint/inject moment), and verify with a constant-time
//! compare so a wrong token can't leak a per-byte timing differential. The one
//! structural difference: these are per-SESSION tokens (one minted per served
//! task-pane load) held in an in-memory FIFO-bounded set, not a map to a DB
//! row — there is no persistent identity behind a bridge session.
//!
//! ## Bounded store (memory-leak fix)
//! A fresh token is minted on every `taskpane.html` GET and `revoke` is not
//! guaranteed (a WebView2 pane can be torn down without a clean WSS close), so an
//! unbounded set would grow for the process lifetime — a slow leak plus an
//! ever-widening set of forever-valid tokens. We cap it at [`MAX_TOKENS`] with
//! FIFO eviction: minting the (N+1)th token evicts the oldest, so at most the N
//! most-recent panes' tokens are ever live. N is generous relative to the number
//! of task panes a user realistically opens between reloads.

use std::collections::{HashSet, VecDeque};
use std::sync::{LazyLock, RwLock};

/// SHA-256 of a session token (32 bytes). The set key, so the plaintext is
/// never retained past `insert`.
pub type TokenHash = [u8; 32];

/// Upper bound on live session tokens. FIFO eviction past this keeps the store
/// from growing unboundedly across many task-pane loads (see module docs).
const MAX_TOKENS: usize = 64;

/// FIFO-bounded set of live session-token hashes: `set` is the O(1) membership
/// index the verify hot-path scans, `order` records insertion order so the
/// oldest hash is evicted first once the store is full.
#[derive(Default)]
struct TokenStore {
    set: HashSet<TokenHash>,
    order: VecDeque<TokenHash>,
}

/// In-memory store of live session-token hashes. Read-mostly (a `verify` per WSS
/// upgrade / POST; writes only on serve-page / revoke), so an `RwLock` keeps
/// the verify hot-path cheap. Poison-recovering like the rest of the tree.
static SESSION_TOKENS: LazyLock<RwLock<TokenStore>> =
    LazyLock::new(|| RwLock::new(TokenStore::default()));

/// SHA-256 a plaintext token → 32 bytes (mirrors `proxy::hash_token`).
fn hash_token(token: &str) -> TokenHash {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

/// Generate a 32-byte URL-safe session token from the OS RNG (mirrors
/// `proxy::generate_proxy_token`). Does NOT register it — use
/// [`new_session_token`] for the mint-and-register path.
pub fn generate_token() -> String {
    use base64::Engine;
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

/// Register a token's hash as a live session. Idempotent; FIFO-evicts the oldest
/// hash once the store would exceed [`MAX_TOKENS`], so the store stays bounded.
pub fn insert(token: &str) {
    let hash = hash_token(token);
    let mut store = SESSION_TOKENS.write().unwrap_or_else(|p| p.into_inner());
    // Idempotent: a re-inserted live token doesn't reorder or grow the store.
    if !store.set.insert(hash) {
        return;
    }
    store.order.push_back(hash);
    while store.order.len() > MAX_TOKENS {
        if let Some(oldest) = store.order.pop_front() {
            store.set.remove(&oldest);
        }
    }
}

/// Mint a fresh session token, register it, and hand back the plaintext to
/// inject into the served page. The plaintext is not retained anywhere else.
pub fn new_session_token() -> String {
    let token = generate_token();
    insert(&token);
    token
}

/// Constant-time verify that `presented` is a live session token. Hashes the
/// presented value and scans the set with a constant-time per-entry compare so
/// a wrong token can't leak a timing differential (belt + suspenders on top of
/// the one-way hash — mirrors `proxy::lookup_token`).
pub fn verify(presented: &str) -> bool {
    let presented_hash = hash_token(presented);
    let store = SESSION_TOKENS.read().unwrap_or_else(|p| p.into_inner());
    for stored in store.set.iter() {
        if constant_time_eq(stored, &presented_hash) {
            return true;
        }
    }
    false
}

/// Revoke a session token (e.g. on socket close). No-op if unknown.
pub fn revoke(token: &str) {
    let hash = hash_token(token);
    let mut store = SESSION_TOKENS.write().unwrap_or_else(|p| p.into_inner());
    if store.set.remove(&hash) {
        store.order.retain(|h| *h != hash);
    }
}

/// Constant-time array equality (no `subtle` dependency; mirrors proxy.rs).
fn constant_time_eq(a: &TokenHash, b: &TokenHash) -> bool {
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_url_safe_43_chars() {
        let t = generate_token();
        assert_eq!(t.len(), 43); // 32 bytes base64url no-pad
        assert!(
            t.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
    }

    #[test]
    fn mint_register_verify_revoke_roundtrip() {
        let t = new_session_token();
        assert!(verify(&t), "freshly minted token verifies");
        // A distinct random token must not verify.
        assert!(!verify(&generate_token()));
        assert!(!verify("definitely-not-a-token"));
        revoke(&t);
        assert!(!verify(&t), "revoked token no longer verifies");
    }

    #[test]
    fn insert_then_verify() {
        let t = generate_token();
        assert!(!verify(&t));
        insert(&t);
        assert!(verify(&t));
        revoke(&t);
    }

    /// The store is FIFO-bounded: minting well past `MAX_TOKENS` never grows it
    /// beyond the cap (the memory-leak guard), and the OLDEST tokens are the ones
    /// evicted first. Both assertions are race-safe against other tests sharing
    /// the process-global store — a bounded cap holds regardless, and additional
    /// concurrent mints can only push the oldest tokens *further* out, never
    /// resurrect them.
    #[test]
    fn store_is_bounded_and_evicts_oldest_first() {
        // Mint 3× the cap so the earliest tokens are guaranteed evicted even if
        // no other test contends.
        let minted: Vec<String> = (0..MAX_TOKENS * 3).map(|_| new_session_token()).collect();

        // The store never exceeds the cap.
        let len = SESSION_TOKENS
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .set
            .len();
        assert!(len <= MAX_TOKENS, "store bounded at {MAX_TOKENS}, got {len}");

        // The oldest 2× cap tokens are all evicted (they precede at least
        // `MAX_TOKENS` later mints from this test alone).
        for t in &minted[..MAX_TOKENS * 2] {
            assert!(!verify(t), "an evicted (oldest) token must not verify");
        }
    }
}

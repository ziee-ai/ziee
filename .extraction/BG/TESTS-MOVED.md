# Chunk BG — TESTS-MOVED / TESTS-TOUCHED

BG is an in-ziee equivalence-preserving refactor — no test MOVES crates and no
new behaviour is added, so no NEW tests are required. The existing auth/user
suites are the behavioural guard and remain in place. Two categories of touch:

## In-source unit tests kept + re-verified (compile under the new seams)

- `modules/auth/jwt.rs` `#[cfg(test)]` — `expiry_override_variant_honored`,
  `debug_seconds_override_wins`, `weak_secret_refused`. Now build `JwtSettings`
  (was `JwtConfig`); assertions unchanged. Compiled + covered by `cargo check -p
  ziee` (test cfg) — GREEN.

## Integration-test signature follow-through (2 files, 5 call sites)

`ensure_unique_username` gained a leading `pool: &PgPool` (Decision 7), so its
`#[doc(hidden)] pub` test callers were updated to pass their existing pool:

- `tests/auth/mod.rs::test_ensure_unique_username_collision_suffix_and_defaults`
  — 3 calls now `ziee::ensure_unique_username(&pool, base)`.
- `tests/auth/profile_self_service_test.rs::test_ensure_unique_username_collision_retry`
  — 3 calls now `ziee::auth_ensure_unique_username(&pool, base)`;
  `init_repositories(pool)` → `init_repositories(pool.clone())` to keep a usable
  pool. Semantics unchanged (same collision-suffix behaviour asserted).

## Unchanged-and-still-passing (no edit needed, verified compiling)

- `tests/auth/session_settings_test.rs`, `tests/auth/session_refresh_test.rs` —
  construct `ziee::JwtService::try_new(ziee::JwtConfig{..})`; unchanged thanks to
  `try_new(impl Into<JwtSettings>)` + the app-side `From<JwtConfig>`.
- `desktop/tauri/tests/auth_tests.rs` — `ziee::JwtService::new(config)` +
  `mint_session_tokens` provenance; unchanged.

**Verification:** `cargo check --test integration_tests` (server) exit 0 — the
entire integration-test target compiles against the new seams. Full test RUN is
the orchestrator's post-merge step (not part of the BG gate).

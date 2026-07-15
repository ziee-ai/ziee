# TESTS — logout session invalidation

Every ITEM-1..12 is covered by ≥1 TEST below. The diff touches a frontend workspace
(`src-app/ui/**`), so `tier: e2e` specs are enumerated (TEST-18..20). The feature introduces **no new
permission**, so no `[negative-perm]` restricted-user e2e is required (A10 not triggered).

No cosmetic tests: every assertion below either drives a real HTTP path against a real server + DB, or
drives the real Zustand store. The only mocked boundary is `@/api-client` in the vitest store tests
(the store's external boundary).

## Unit — Rust

- **TEST-1** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/auth/jwt.rs` — asserts: an access token minted with `token_version = 7` round-trips through `validate_access_token` carrying `ver == Some(7)`, and a refresh token minted alongside carries `ver == None`.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/auth/jwt.rs` — asserts: a hand-encoded `Claims`-shaped JSON with NO `ver` field deserializes successfully with `ver == None` — pinning the `#[serde(default)]` back-compat contract that keeps pre-deploy tokens working.
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/auth/jwt_extractor.rs` — asserts: `verify_token_version` — the single comparison rule shared by both read paths — returns Ok when `Some(3)` vs db `3`; Err(401 `SESSION_REVOKED`) when `Some(3)` vs db `4`; and **Ok when `None` vs db `0`** (a pre-migration token against a default column).

## Integration — Rust (`src-app/server/tests/`)

- **TEST-4** (tier: integration) [covers: ITEM-1, ITEM-8] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: **THE CORE GAP** — register → `/auth/me` 200 → `POST /auth/logout` 204 → `/auth/me` with the SAME (unexpired) access token → **401 with error_code `SESSION_REVOKED`**.
- **TEST-5** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: after logout, the two bare-`JwtAuth` routes that neither `me` nor `RequirePermissions` covers — `GET /api/onboarding/progress` and `GET /api/hub/installed` — both 401. Proves the extractor-level coverage claim.
- **TEST-6** (tier: integration) [covers: ITEM-2, ITEM-5] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: after logout, a `RequirePermissions`-gated route returns 401 with the old token — using **`GET /api/conversations`**, i.e. the literal reported leak (the ex-admin's conversations). Exercises the folded `get_by_id_with_token_version` read path.
- **TEST-7** (tier: integration) [covers: ITEM-6, ITEM-7] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: logout → login again **within the same wall-clock second** (no sleep) → `/auth/me` **200**. The executable proof that the rejected `sessions_revoked_at`-vs-`iat` design is broken and the counter is not.
- **TEST-8** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: **LOGOUT ATOMICITY** — with a `BEFORE UPDATE` trigger on `refresh_tokens` that `RAISE EXCEPTION`s (installed via `sqlx::PgPool::connect(&server.database_url)` on this test's own unique DB), `POST /auth/logout` fails → `users.token_version` is **UNCHANGED**, the access token still 200s, and (trigger dropped) the refresh token still rotates. Both-or-neither: no bump-without-revoke window in which a held refresh token re-mints a valid access token past logout.
- **TEST-9** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: **read-before-claim** — a refresh whose rotation has already been claimed, racing a logout, yields a token stamped with the OLD `ver` → `/auth/me` 401. Asserts the ordering invariant, not a sleep.
- **TEST-10** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: user A's logout leaves user B's access token working (the bump is per-user, not global).
- **TEST-11** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: a hand-minted `ver`-less token (signed with the server's secret) still authenticates against a `token_version = 0` user → 200. Pins the zero-forced-logout-at-deploy story.
- **TEST-12** (tier: integration) [covers: ITEM-8] file: `src-app/server/tests/sync/delivery_test.rs` — asserts: logout fans a `Session` signal to the user's OTHER devices — two `SyncProbe`s for one user; logout carrying probe-1's `X-Sync-Connection-Id` → probe-2 `expect_event("session","update")` AND probe-1 `expect_silence` (origin suppression). Mirrors `deactivating_a_user_mid_stream_closes_their_sync_stream`.
- **TEST-13** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/auth/mod.rs` — asserts: `test_auth_logout` no longer documents the bug as expected ("JWT is stateless, so the token will still work after logout") but asserts the post-logout access token is rejected.

## Unit — frontend (vitest, jsdom; `vitest.config.ts` already includes `src/**/*.store.test.ts`)

- **TEST-14** (tier: unit) [covers: ITEM-10, ITEM-11] file: `src-app/ui/src/modules/auth/Auth.store.test.ts` — asserts: `logoutUser()` wipes the session INCLUDING `permissions` (`[]`) and `hasPassword` (`false`) — not just `token`/`user` — and calls `window.location.reload` exactly once.
- **TEST-15** (tier: unit) [covers: ITEM-10] file: `src-app/ui/src/modules/auth/Auth.store.test.ts` — asserts: **desktop safety** — with a `refreshFallback` registered, `logoutUser()` does NOT reload (the desktop bundle is never bounced to a login page), while still clearing state.
- **TEST-16** (tier: unit) [covers: ITEM-10] file: `src-app/ui/src/modules/auth/Auth.store.test.ts` — asserts: a terminal 401 from `/auth/refresh` (no fallback) tears down and reloads once, leaving `isAuthenticated === false`.
- **TEST-17** (tier: unit) [covers: ITEM-10] file: `src-app/ui/src/modules/auth/Auth.store.test.ts` — asserts: **desktop permanence** — a terminal refresh 401 WITH a `refreshFallback` that re-mints does NOT reload and leaves `token` non-null. Pins that the existing `if (refreshFallback)` guard still precedes the teardown.
- **TEST-18** (tier: unit) [covers: ITEM-11] file: `src-app/ui/src/modules/auth/Auth.store.test.ts` — asserts: `setAuthFromAutoLogin` clears the PREVIOUS identity's `permissions` at the same tick it flips `isAuthenticated: true` — no authenticated render window with a foreign permission set.

## E2E — Playwright (`--workers=1`; never `waitForLoadState('networkidle')` — the SSE stream never settles)

- **TEST-19** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/auth/logout.spec.ts` — asserts: logging out via the profile dropdown lands on the login form AND leaves `localStorage['auth-storage']` parsed `state.token === null` (so a reloaded tab cannot resurrect the session).
- **TEST-20** (tier: e2e) [covers: ITEM-8, ITEM-10] file: `src-app/ui/tests/e2e/sync/session-sync.spec.ts` — asserts: **THE REPORTED CROSS-TAB SYMPTOM** — tab 1 and tab 2 in the SAME context (`page.context().newPage()`, so shared localStorage + cookies = true tabs, NOT `browser.newContext()`); logout in tab 1 → tab 2 shows the login form **with no reload driven by the test**. Exercises the whole chain: bump → publish → SSE → `/auth/me` 401 → interceptor → refresh 401 → teardown.
- **TEST-21** (tier: e2e) [covers: ITEM-4, ITEM-5] file: `src-app/ui/tests/e2e/auth/logout.spec.ts` — asserts: **the server-side backstop, independent of the SSE signal** — tab 2 with `route('**/api/sync/subscribe', r => r.abort())` before navigating; logout in tab 1; tab 2's next navigation/click hits the login form rather than admin data. This is what makes cutting BroadcastChannel defensible.

## Added in DRIFT-1 (impl-wins)

- **TEST-22** (tier: integration) [covers: ITEM-13] file: `src-app/server/tests/mcp/builtin_test_connection_test.rs` — asserts: after a logout has bumped the user's epoch (0→1), a re-logged-in user's built-in (loopback) MCP servers still pass `POST /mcp/system-servers/test-connection`. Proves the internal service-to-service JWT is minted with the CURRENT `token_version`; a defaulted/stale epoch would 401 every built-in tool call (memory, files, web_search, code_sandbox…) for any user who had ever logged out. Extends the existing enumerate-every-built-in regression test, so a future built-in inherits the coverage.

## Added in DRIFT-2 (blind-audit fixes)

- **TEST-23** (tier: integration) [covers: ITEM-14] file: `src-app/server/tests/auth/session_refresh_test.rs` — asserts: **no refresh token survives a concurrent logout**. Drives 12 rounds of a real concurrent `/auth/refresh` vs `/auth/logout` over HTTP, then asserts the invariant that must hold whoever wins: zero rows in `refresh_tokens` with `revoked_at IS NULL` for the user, any token the refresh handed back 401s, its successor is not replayable, and the original access token is dead. This is what catches the READ COMMITTED phantom that let a successor escape revocation.
- **TEST-24** (tier: integration) [covers: ITEM-15] file: `src-app/server/tests/sync/delivery_test.rs` — asserts: an ALREADY-OPEN sync stream is closed by the server after a logout (`SyncProbe::expect_closed`), with the logout deliberately sent WITHOUT `X-Sync-Connection-Id` so the origin-suppressed fan-out cannot be what closes it — only the server-side epoch re-check can. Uses the `SYNC_RECHECK_TICK_MS` debug seam.

**Renamed for honesty (FIX_ROUND-1):** TEST-9's spec is now `test_refresh_then_logout_kills_the_refreshed_session` — it is sequential and does NOT pin the read-before-claim ordering it previously claimed; TEST-23 covers the real interleaving. **TEST-21 rewritten:** it now uses an independent browser context with its own login + its own stored token (the same-context version passed with the server-side feature fully reverted).

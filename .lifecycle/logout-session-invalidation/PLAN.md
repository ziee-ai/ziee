# PLAN — SECURITY: logout does not fully invalidate the session

Fixes a reported security bug: with an admin in two tabs, logout in tab 1 leaves (a) the same tab
showing the admin's data after logging in as a non-admin, and (b) tab 2 fully admin. Root cause is
three independent gaps — a stateless access JWT that is never revocation-checked, a client wipe that
leaves per-user state behind, and no cross-tab signal.

Approved by the human on 2026-07-15, including two review fixes (logout atomicity; folded
version read). Full reasoning: `/home/khoi/.claude/plans/read-the-task-file-fluttering-anchor.md`.

## Items

- **ITEM-1**: Migration `00000000000158_add_users_token_version.sql` — `ALTER TABLE users ADD COLUMN IF NOT EXISTS token_version INTEGER NOT NULL DEFAULT 0` + `COMMENT ON COLUMN` documenting that logout bumps it, it is stamped as the access token's `ver` claim, and a mismatch is a 401. No index (always read by the `users` PK).
- **ITEM-2**: `user/repository.rs` — `get_by_id_with_token_version(id) -> Option<(User, i32)>` (explicit column list into a NON-serialized internal row, so `token_version` never lands on the `Serialize + JsonSchema` `User` struct) and `get_token_version(id) -> Option<i32>` (`query_scalar!`, for the bare-`JwtAuth` path).
- **ITEM-3**: `auth/jwt.rs` — add `#[serde(default, skip_serializing_if = "Option::is_none")] pub ver: Option<i32>` to `Claims`; stamp it in `generate_access_token`; thread `token_version` through `generate_tokens_with_jti_expiry` / `reissue_tokens_for_jti`. Refresh tokens keep `ver: None` (the `refresh_tokens` whitelist already revokes them). Mirrors the existing optional-`jti` claim pattern.
- **ITEM-4**: `auth/jwt_extractor.rs` — pure `verify_token_version(claims_ver: Option<i32>, db_version: i32)` (THE single comparison rule: `claims_ver.unwrap_or(0) != db_version` → 401 `SESSION_REVOKED`) + `assert_token_version_current(claims)` (scalar read → `verify`), carrying an INVARIANT doc comment naming both read paths. Call from `JwtAuth`; call from `OptionalJwtAuth` returning anonymous on mismatch.
- **ITEM-5**: `permissions/extractors.rs` — in `extract_authenticated_user`, swap `get_by_id` → `get_by_id_with_token_version` and call `verify_token_version` BEFORE the `is_active` check. One query on the hot path, no extra round-trip. Covers every `RequirePermissions` / `RequireAdmin` route.
- **ITEM-6**: `auth/refresh_tokens.rs` — new `end_session_atomically(pool, user_id) -> Result<i32, AppError>`: `pool.begin()` → bump `users.token_version` → revoke all the user's refresh tokens → `tx.commit()`. Both commit or neither, closing the partial-logout window where a surviving refresh token re-mints a valid access token stamped with the NEW `ver`. `revoke_all_for_user`'s signature stays UNCHANGED (its 2 other callers stay out of scope).
- **ITEM-7**: `auth/refresh_tokens.rs` — `mint_session_tokens` reads `token_version` and stamps it, so all 8 login-shaped call sites are unchanged and desktop `auto_login` self-heals for free.
- **ITEM-8**: `auth/handlers.rs` `logout` — take `origin: SyncOrigin`, call `end_session_atomically`, and `sync_publish(SyncEntity::Session, SyncAction::Update, user_id, Audience::owner(user_id), origin.0)` AFTER the commit (a tab racing to `/auth/me` on the signal must see the bump). Rewrite the stale "revocation deferred" doc comment.
- **ITEM-9**: `auth/handlers.rs` `refresh` — read `token_version` BEFORE `claim_rotation_and_register` (not after), closing the read-after-claim window in which a refresh racing a logout mints a token carrying the new `ver` that survives the logout.
- **ITEM-10**: `ui/src/modules/auth/Auth.store.ts` — add `tearDownSession()` (wipe incl. `permissions`, then `window.location.reload()`, so zero prior-user bytes survive by construction); call it from `doRefresh`'s terminal-401 branch (AFTER the existing `if (refreshFallback)` desktop guard — do not move it) and from `logoutUser` gated on `!refreshFallback`.
- **ITEM-11**: `ui/src/modules/auth/Auth.store.ts` — add `permissions: []` + `hasPassword: false` to ALL FOUR session-wipe sites (`doRefresh` terminal 401, `authenticateUser` catch, `logoutUser`, `initAuth` catch), and `permissions: []` to `setAuthFromAutoLogin` (which today flips `isAuthenticated: true` while the PREVIOUS user's permissions are still in state).
- **ITEM-12**: `tests/auth/mod.rs` — replace the comment that pins the bug as expected behavior ("JWT is stateless, so the token will still work after logout … you'd need a token blacklist or short expiry") with a real post-logout 401 assertion.
- **ITEM-13**: *(added in DRIFT-1, impl-wins)* `mcp/client/manager.rs` — `generate_short_lived_jwt` mints the internal service-to-service access token used by EVERY built-in MCP server call (and `resource_link` loopback fetches). It is a third `Claims` construction site outside `jwt.rs` that the plan missed. Its routes are gated by `RequirePermissions`, so a defaulted `ver` would 401 every built-in tool call for any user who had ever logged out. Mint it with the user's CURRENT epoch: `generate_short_lived_jwt` gains a `token_version` param, `inject_builtin_context_headers` becomes async (2 call sites: `manager.rs`, `mcp/handlers/test_connection.rs`), and `resource_link.rs` reads the epoch before minting.
- **ITEM-14**: *(added in DRIFT-2, from the blind audit — HIGH)* `auth/refresh_tokens.rs` — `claim_rotation_and_register` must take the `users` row lock (`SELECT token_version … FOR SHARE`) as the FIRST statement of its transaction. Without it, under READ COMMITTED a successor refresh token INSERTed-but-not-COMMITTed is never scanned by logout's `UPDATE refresh_tokens WHERE user_id = $1`, so it survives the logout and can re-mint an access token carrying the NEW epoch — defeating the feature. Lock order stays `users → refresh_tokens` in both paths (no deadlock).
- **ITEM-15**: *(added in DRIFT-2, from the blind audit — MEDIUM)* `sync/handlers.rs` + `chat/stream/handler.rs` — the periodic re-check on an already-open SSE stream must also verify the access-token epoch, so a logout ends live streams instead of leaving them delivering (for the chat stream, actual assistant content) until the token's `exp`. Free: both loops already load the user row; swap `get_by_id` → `get_by_id_with_token_version`.

## Files to touch

- `src-app/server/migrations/00000000000158_add_users_token_version.sql` *(new)*
- `src-app/server/src/modules/user/repository.rs`
- `src-app/server/src/modules/auth/jwt.rs`
- `src-app/server/src/modules/auth/jwt_extractor.rs`
- `src-app/server/src/modules/auth/refresh_tokens.rs`
- `src-app/server/src/modules/auth/handlers.rs`
- `src-app/server/src/modules/permissions/extractors.rs`
- `src-app/server/src/modules/mcp/client/manager.rs` *(ITEM-13)*
- `src-app/server/src/modules/mcp/handlers/test_connection.rs` *(ITEM-13, async call site)*
- `src-app/server/src/modules/mcp/resource_link.rs` *(ITEM-13)*
- `src-app/server/tests/mcp/builtin_test_connection_test.rs` *(ITEM-13 regression)*
- `src-app/ui/src/modules/auth/Auth.store.ts`
- `src-app/ui/src/modules/auth/Auth.store.test.ts` *(new)*
- `src-app/server/tests/auth/session_refresh_test.rs`
- `src-app/server/tests/auth/mod.rs`
- `src-app/server/tests/sync/delivery_test.rs`
- `src-app/ui/tests/e2e/auth/logout.spec.ts`
- `src-app/ui/tests/e2e/sync/session-sync.spec.ts`

NOT touched (deliberate): `refreshFromSync`, `api-client/core.ts`, `AuthGuard*.tsx`,
`core/store-kit.ts`, `core/module-system/store.ts`, the other 122 `*.store.ts` files, any desktop
file, and `revoke_all_for_user`'s signature.

## Patterns to follow

| Piece | Closest existing module to MIRROR |
|---|---|
| Migration (add column to `users` + comment) | `00000000000064_add_users_password_changed_at.sql` — same table, same `ADD COLUMN IF NOT EXISTS` + `COMMENT ON COLUMN` shape |
| Repository read fns | `user/repository.rs::get_by_id:24-39` — explicit column list, `fetch_optional`, `map_err(AppError::database_error)` |
| Transactional bump+revoke | `auth/refresh_tokens.rs::claim_rotation_and_register:163-206` — SAME FILE: `pool.begin()` → `execute(&mut *tx)` → `tx.commit()`; the repo's canonical multi-statement-atomicity shape |
| Optional claim + `#[serde(default)]` | `auth/jwt.rs::Claims.jti:19-25` — including the doc-comment rationale for why it's optional |
| DB read inside the mint path | `auth/refresh_tokens.rs::session_expiries:19-33` — the precedent that `mint_session_tokens` already does DB I/O with a graceful fallback |
| `sync_publish` from a handler | `user/handlers/user.rs:461-469` (admin password reset) — `SyncEntity::Session` / `SyncAction::Update` / `Audience::owner(uid)` / `origin.0` |
| Integration test: two-device SSE + origin suppression | `tests/sync/delivery_test.rs:126-160` — `SyncProbe` + `expect_event` / `expect_silence` / `expect_closed` |
| Integration test: direct DB access | `tests/workflow_mcp/mod.rs:131` — `sqlx::PgPool::connect(&server.database_url)`; safe because each test owns a unique per-test DB |
| Frontend store unit test | `chat/stores/ChatHistory.store.test.ts:18-40` — `vi.hoisted` + `vi.mock('@/api-client')`; `vitest.config.ts` includes `src/**/*.store.test.ts` (jsdom) |
| E2E two-context | `tests/e2e/sync/session-sync.spec.ts:46-141` — open the 2nd context BEFORE the mutation; NEVER `waitForLoadState('networkidle')` (the SSE stream never settles). **Use `page.context().newPage()` for a true cross-TAB test (shared localStorage + cookies), not `browser.newContext()`** |
| E2E logout click-path | `tests/e2e/auth/logout.spec.ts:14-33` — `user-profile-widget` → `userprofile-menu-dropdown-item-logout` → assert `auth-login-username` |

## UI-surface checklist

**Not applicable — this feature adds no UI surface.** The only frontend change is behavioral, inside
`Auth.store.ts` (session teardown). It renders no new page/drawer/card/panel, adds no new
conditional render state (so `check:state-matrix` needs no new gallery cell), introduces no new
permission, and changes no layout. Existing surfaces (`AuthPage`, the profile dropdown) are reached
by exactly the paths they are reached by today; the change is that a terminal teardown now reloads
the document before `AuthGuard` renders `AuthPage`.

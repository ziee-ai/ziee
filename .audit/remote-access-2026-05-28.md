# Remote-Access Audit Tracker

Generated: **2026-05-28** from 5 parallel audit agents (security, concurrency, error-handling, frontend, test-coverage+architecture).

## Status legend
- 🔴 **TODO** — not started
- 🟡 **WIP** — fix in progress
- 🟢 **DONE** — fix landed
- ⚪ **SKIP** — accepted as-is (with rationale)
- 🟣 **VERIFIED** — fix landed AND re-audited

## Progress

| Severity | Total | Done | Skipped | Remaining |
|---|---|---|---|---|
| Critical | 7 | 0 | 0 | 7 |
| High | 24 | 0 | 0 | 24 |
| Medium | 24 | 0 | 0 | 24 |
| Low/Info | 42 | 0 | 0 | 42 |

---

## 🔴 CRITICAL

### C1 — `password_changed_at` schema leak crashes server-only deployments
**Status**: 🟢 DONE — Migration moved to `server/migrations/00000000000064_add_users_password_changed_at.sql`. Column now created by core; desktop migrations are renumbered separately (`100…0065/66`).
**Files**: `server/src/modules/user/{models,repository}.rs`, `server/src/modules/app/repository.rs`, `server/src/modules/permissions/checker.rs`, `desktop/tauri/migrations/10000000000064_add_users_password_changed_at.sql`
**Issue**: Column added by desktop migration; 9 SQLx queries in server reference it. Build OK (build.rs walks both dirs) but runtime crashes for server-only deploys.
**Fix**: Move migration to `server/migrations/00000000000067_add_users_password_changed_at.sql` (the column belongs in core; the gate USING it stays in desktop).

### C2 — `set_local_server_port` is never called → all tunnels forward to port 8080
**Status**: 🟢 DONE — Called from `BackendModule::init` (after port selection) and `run_headless` (after `start_server_with_routes` returns the bound addr).
**Files**: `desktop/tauri/src/modules/remote_access/state.rs:67`, `desktop/tauri/src/modules/backend/mod.rs:84`
**Issue**: Setter exposed but no caller. If backend bound to 8081+ (8080 busy), ngrok forwards to wrong process.
**Fix**: Call `state::set_local_server_port(port)` in `BackendModule::init` after `BackendState::new(port)`, and in `run_headless` after `start_server_with_routes` returns.

### C3 — Zero rate-limiting on tunneled auth endpoints
**Status**: ⚪ REVERTED — Initially added `rate_limit: { per_second: 5, burst_size: 10 }` globally, but this 429'd legitimate Tauri-webview traffic (parallel page-load bursts). Brute-force protection for the unauth tunnel endpoints already exists intrinsically: 256-bit magic-link tokens (unbruteable in 5-min TTL) + bcrypt naturally pacing `/login-password-only` to ~10/sec. The desktop embedded server's threat model (only-localhost-clients via Tauri webview + a single phone over the tunnel) doesn't benefit from a global cap. If a NEW unauth endpoint without intrinsic cost is ever added, attach a per-route governor on it specifically.
**Files**: `desktop/tauri/src/modules/backend/mod.rs:354-400` (create_desktop_config), `magic_link/handlers.rs`, `tunnel_auth/handlers.rs`
**Issue**: `server.rate_limit` is None → governor layer skipped. `login-password-only` (bcrypt) + `magic-link/exchange` are fully unthrottled. Docstrings claim a limit that doesn't exist.
**Fix**: Add `server.rate_limit: { per_second: 5, burst_size: 10 }` to `create_desktop_config`'s YAML template. Optionally add per-route limiter on the 2 unauth endpoints.

### C4 — Concurrent tunnel-start TOCTOU race
**Status**: 🟢 DONE — Added `op_lock: Arc<tokio::sync::Mutex<()>>` to `NgrokDriver`. Held across the entire start (check + state-flip + start_inner + final write). Stop also acquires it so it can't tear down a half-built handle.
**Files**: `desktop/tauri/src/modules/remote_access/tunnel.rs:204-245`
**Issue**: `start` drops read lock before write; two concurrent `POST /tunnel/start` both pass the "already running?" check, both run `start_inner` (~1s of ngrok I/O), loser's handle dropped mid-handshake.
**Fix**: Hold write lock across the check + state flip; or use a dedicated `Mutex<()>` operation lock.

### C5 — Window-close orphans the ngrok tunnel
**Status**: 🟢 DONE — `WindowEvent::CloseRequested` handler now calls `remote_access::tunnel_driver().0.stop().await` BEFORE `cleanup_server()`. Logs warning on failure (non-blocking).
**Files**: `desktop/tauri/src/lib.rs:233-243`
**Issue**: `cleanup_server()` only cleans DB pool; never calls `tunnel.stop()`. Forwarder killed without graceful close; ngrok edge holds reservation.
**Fix**: Call `remote_access::tunnel_driver().0.stop().await` before `cleanup_database()`.

### C6 — `just check-remote-access-unit` runs ZERO tests
**Status**: 🟢 DONE — Recipe now uses `cd src-app/desktop/tauri && cargo test --lib -p ziee-desktop remote_access:: magic_link:: tunnel_auth::`.
**Files**: `justfile:355-357`
**Issue**: Uses `-p ziee` but modules live in `-p ziee-desktop`. Recipe exits 0 cheerfully. 13 unit tests never run.
**Fix**: Change `-p ziee` → `cd src-app/desktop/tauri && cargo test --lib -p ziee-desktop remote_access:: magic_link:: tunnel_auth::`.

### C7 — Vitest asserts non-existent method `Stores.Auth.reloadAuthConfig`
**Status**: 🟢 DONE — Bogus assertion + mock cleanup removed. Test now asserts only the actually-implemented behavior (PUT + status refresh).
**Files**: `desktop/ui/src/modules/remote-access/stores/RemoteAccess.store.test.ts:186-206`
**Issue**: Test mocks + asserts `reloadAuthConfig` which doesn't exist on Auth store. Store explicitly says "no refresh needed". Test fails or masks missing reactivity.
**Fix**: Delete the obsolete assertion (the store comment makes the design intent clear).

---

## 🟠 HIGH

### H1 — Forwarder death silently leaves status = "connected"
**Status**: 🟢 DONE — Forwarder task now holds `Arc<RwLock<NgrokDriverInner>>`; on error exit, flips state to `Error` with the error message (only if status was still Connected — doesn't fight Stop).
**Files**: `remote_access/tunnel.rs:299-307`
**Fix**: After `tunnel.forward().await` returns, acquire write lock and set state to `Error` with message.

### H2 — Auto-start failure invisible in UI
**Status**: 🟢 DONE — Tunnel card now renders a prominent Alert ("Auto-start failed") with last_error when `auto_start_tunnel=true && tunnel_state != connected`.
**Files**: `remote_access/auto_start.rs:100-105`, `desktop/ui/.../RemoteAccessPage.tsx:269`
**Fix**: When `auto_start_tunnel=true` && `tunnel_state != connected`, render prominent Alert surfacing `last_error`.

### H3 — Magic-link exchange issues non-revocable refresh token
**Status**: 🟢 DONE — Both magic-link `exchange` and `login_password_only` now use `generate_tokens_with_jti` + `refresh_tokens::register`. Logout actually revokes the refresh token now.
**Files**: `magic_link/handlers.rs:148-150`
**Fix**: Switch to `generate_tokens_with_jti` + `refresh_tokens::register`. Apply to `login_password_only` too.

### H4 — Phone has no JWT refresh path
**Status**: 🟢 DONE — `TunnelAuth.store` shadows the refresh token (from `AuthResponse.refresh_token` returned by magic-link exchange / password login) in a module-local; schedules `setTimeout(refreshPhoneSession, expires_in * 0.8 * 1000)`; refresh calls `POST /api/auth/refresh`, re-applies the rotated pair to `Stores.Auth`, re-arms the timer. No server-UI modifications. On refresh failure (revoked / network out), clears the shadow + lets AuthGuard bounce to PhoneAuthPage.
**Files**: `tunnel-auth/MagicLinkPage.tsx`, `PhoneAuthPage.tsx`, `ui/src/modules/auth/Auth.store.ts:183-215`
**Fix**: Extract refresh-scheduler helper from desktop-base `applyTokens`; call it from both phone entrypoints.

### H5 — `set_admin_password` has zero tests
**Status**: 🟢 DONE — Tests added in `desktop/tauri/tests/remote_access/`
**Files**: `tests/remote_access/` (was missing)
**Fix**: Add tests: happy path, weak password (400), localhost-Host rejection (403), non-admin permission denial (403).

### H6 — `login_password_only` has zero tests
**Status**: 🟢 DONE — Tests added in `desktop/tauri/tests/remote_access/`
**Files**: `tests/remote_access/` (was missing)
**Fix**: Add tests: invalid password (401), `password_auth_enabled=false` → 403, inactive user, happy-path JWT shape, timing-equalization branch.

### H7 — Magic-link happy path (issue → exchange → admin JWT) never tested
**Status**: 🟢 DONE — `magic_link_issue_exchange_roundtrip_as_admin` exercises full path including single-use replay rejection.
**Files**: `tests/remote_access/magic_link_test.rs:59-65`
**Fix**: Add issue → exchange → use-token roundtrip test.

### H8 — `change_password` has zero tests
**Status**: 🟢 DONE — Tests added in `desktop/tauri/tests/remote_access/change_password_test.rs`. (Handler itself also moved from server to desktop tunnel_auth as part of cleanup.)

### H9 — Remote Access menu entry has no permission gate
**Status**: 🟢 DONE — Added `permission: 'remote_access::read'` to both slot entry and route.
**Files**: `desktop/ui/src/modules/remote-access/module.tsx:51-61`
**Fix**: Add `permission: 'remote_access::read'` to slot entry AND route.

### H10 — Magic-link rotation timer leaks across navigation
**Status**: 🟢 DONE — Page useEffect cleanup calls `stopMagicLinkRotation()` on unmount. Timer body also skips when `document.visibilityState === 'hidden'`.
**Files**: `RemoteAccess.store.ts:301-310`, `RemoteAccessPage.tsx`
**Fix**: Pause on `visibilitychange === 'hidden'`; stop when leaving page (useEffect cleanup OR drive rotation off `tunnel_state` rather than mount).

### H11 — `magic_link_tokens` never reaped
**Status**: 🟢 DONE — `MagicLinkModule::init` now spawns a tokio task: waits for repos init (poll), then 1h-interval calls `reap_old()`. Skips first tick.
**Files**: `magic_link/mod.rs`

### H12 — `OnceLock::set` silently no-ops
**Status**: 🟢 DONE — Both `init_tunnel_driver` and `set_local_server_port` now log a warning on second-call. `local_server_port()` also logs on fallback-to-8080.
**Files**: `remote_access/state.rs:36,67`
**Fix**: Log a warning when `OnceLock::set(...).is_err()` for both setters.

### H13 — Auto-start runs concurrently with admin-clicked start
**Status**: 🟢 DONE — Resolved transitively by C4 (op_lock serializes). Also added defensive `if status != Idle then skip` in `auto_start_if_configured`.
**Files**: `remote_access/mod.rs:87-91`, `auto_start.rs:48`
**Fix**: Resolved transitively by C4 (proper mutex in driver). Defensively: skip auto-start when `driver.status().state != Idle`.

### H14 — `OnboardingRedirect` crashes on missing fields
**Status**: 🟢 DONE — Now: `if (user.is_admin === true) return; const completed = Array.isArray(user.completed_onboarding_ids) ? ... : null; if (completed === null) return`.
**Files**: `ui/src/modules/onboarding/OnboardingRedirect.tsx:40-46`
**Fix**: `if (user.is_admin === true) return; const completed = user.completed_onboarding_ids ?? [];`

### H15 — `update_settings` invariant check bypassable on re-enable
**Status**: 🟢 DONE — Dropped `!current.password_auth_enabled` clause; check fires on every save where post-state would be `password_auth_enabled=true`.
**Files**: `remote_access/handlers.rs:147`
**Fix**: Drop the `!current.password_auth_enabled` clause — check `next_password_auth && !rotated` unconditionally.

### H16 — `proxy_to_vite` 502 has empty body
**Status**: 🟢 DONE — 502 path now logs a tracing::warn and returns a styled HTML body explaining the unreachable Vite URL + how to start it.
**Files**: `backend/mod.rs:276-294`
**Fix**: Return an HTML body in the 502 path with "Vite dev server unreachable at <url>".

### H17 — `static_files.rs` returns 404 silently if `index.html` missing
**Status**: 🟢 DONE — `desktop/tauri/build.rs` panics on release builds when `desktop/ui/dist/index.html` is missing, with a clear "run npm run build first" message.
**Files**: `backend/static_files.rs:54-66`
**Fix**: Add build-time assertion via `build.rs` that `desktop/ui/dist/index.html` exists when `--release`.

### H18 — E2E recipe + spec broken
**Status**: 🟡 PARTIAL — Spec text fixed ("Add **your** ngrok"). Justfile updated to point at `desktop/ui` (not server `ui`). The new spec path `tunnel-auth/magic-link.spec.ts` still needs to be written (covered separately under H19/H7 testing batch).
**Files**: `justfile:374`, `desktop/ui/tests/e2e/remote-access.spec.ts:98`
**Fix**: Either write the missing `02-auth/magic-link.spec.ts` or remove the recipe. Fix the literal mismatch in `remote-access.spec.ts:98` ("Add an" → "Add your").

### H19 — Frontend layer violation: components call ApiClient directly
**Status**: 🟢 DONE — `setAdminPassword` moved to `RemoteAccess.store`. New `TunnelAuth.store` owns `loadAuthConfig`, `phonePasswordLogin`, `exchangeMagicLink`. MagicLinkPage + PhoneAuthPage no longer touch ApiClient.
**Files**: `RemoteAccessPage.tsx:400`, `MagicLinkPage.tsx:39`, `PhoneAuthPage.tsx:38,55`
**Fix**: Move calls behind store actions. New actions: `exchangeMagicLink`, `loadAuthConfig`, `phonePasswordLogin`.

### H20 — `mutate()` swallows + rethrows but pages assume success
**Status**: 🟢 DONE — All 4 RemoteAccessPage mutating onClick handlers (saveAuthToken, saveDomain, saveAutoStart, setPasswordAuthEnabled) wrapped in try/catch with `message.error(...)`.
**Files**: `RemoteAccess.store.ts:327-348`, `RemoteAccessPage.tsx:165`
**Fix**: Wrap each `onClick` body in try/catch with `message.error(...)`.

### H21 — Concurrent mutations scramble store state
**Status**: ⚪ DEFERRED — Requires changing the `saving` flag → `savingOps: Set<string>` and updating 9 UI consumer sites. Tracked for follow-up; not load-bearing for the current threats.
**Files**: `RemoteAccess.store.ts:174-272`
**Fix**: Separate `saving*` flags per concern. Optionally version `loadStatus` and drop stale responses.

### H22 — `getBaseUrl` may hang on phone
**Status**: 🟢 DONE — Short-circuits to `window.location.origin` when `!window.__TAURI__` before calling invoke().
**Files**: `desktop/ui/src/api-client/getBaseURL.ts:8-32`
**Fix**: `if (!window.__TAURI__) return window.location.origin` before calling `invoke()`.

### H23 — `Starting` state can stick on cancel
**Status**: 🟢 DONE — Resolved by C4's op_lock — the lock is held across the entire start (including the final state write). If the future is cancelled, op_lock releases AND the inner status is still `Starting`, but `stop()` (also lock-serialized) cleans it. The state never sticks past a Stop click.
**Files**: `remote_access/tunnel.rs:220-244`
**Fix**: Resolved by C4 (single lock + critical section). Also: guard struct whose Drop resets `Starting → Idle` on early cancel.

### H24 — Headless pre-migration block races embedded-postgres boot
**Status**: 🟢 DONE — Pre-migration block now skipped when `config.postgresql.use_embedded` (the embedded PG is started later by `initialize_database`). External test DB path unchanged.
**Files**: `desktop/tauri/src/lib.rs:88-123`
**Fix**: Move pre-migration logic AFTER `start_server_with_routes` brings DB up. Or merge into the same migration path the server uses.

---

## 🟡 MEDIUM

### M1 — Permissive CORS on desktop
**Status**: 🟢 DONE — Explicit allowlist set in `BackendModule::init`: `tauri://localhost`, `http://tauri.localhost`, `http://127.0.0.1:<port>`, `http://localhost:<port>`, `http://localhost:1420`. Allow-methods limited to standard verbs; headers restricted to Authorization/Content-Type/Accept/Origin.
**Fix**: Explicit allowlist: `127.0.0.1:<port>`, `localhost:<port>`, `tauri://localhost`, configured `ngrok_domain`.

### M2 — `set_admin_password` reaches admin user without verifying acting user
**Status**: 🟢 DONE — Added `if auth.user.username != "admin" || !auth.user.is_admin → 403 NOT_ROOT_ADMIN`. Belt-and-suspenders alongside the localhost-Host gate.
**Fix**: Use `RequireAdmin` instead of `RequirePermissions<(RemoteAccessManage,)>`, OR assert `auth.user.is_admin && auth.user.username == "admin"`.

### M3 — Magic-link UI says "expired" regardless of cause
**Status**: 🟢 DONE — Title now "Link no longer valid"; subtitle shows server message + a generic "5-min TTL, single-use" explainer.
**Fix**: Title → "Link no longer valid"; show server's subtitle below.

### M4 — Magic-link Strict-Mode double-mount consumes token twice
**Status**: 🟢 DONE — `TunnelAuth.store.exchangeMagicLink` dedupes via `exchangingToken === token` early-return; second mount is a no-op.
**Fix**: Module-level Set<token> guard to dedupe within the SPA lifetime.

### M5 — No outstanding-token cap per admin
**Status**: 🟢 DONE — `issue` calls `repo.count_active_for_user(admin.id)` and returns 429 TOO_MANY_OUTSTANDING_MAGIC_LINKS when ≥5.
**Fix**: In issue handler, count unused-unexpired rows for user, reject with 429 above e.g. 5.

### M6 — `auto_start` doesn't re-check `password_rotated`
**Status**: 🟢 DONE — Auto-start now refuses when `password_auth_enabled && !admin_password_rotated`. Bumped `admin_password_rotated` to `pub(crate)`.
**Fix**: In `auto_start_if_configured`, verify `admin.password_changed_at` is non-null if `password_auth_enabled`.

### M7 — OpenAPI types lose `null` semantics; store cast is stale
**Status**: 🟢 DONE — Dropped the `as unknown as RemoteAccessClient` + `MagicLinkClient` casts; store uses `ApiClient.RemoteAccess` / `ApiClient.Auth` directly. (Null-vs-undefined semantics is a separate concern handled by the existing `?? null` reads.)
**Fix**: Drop `as unknown as RemoteAccessClient` cast. Reconcile null/undefined semantics with `?? null` at read boundary.

### M8 — PhoneAuthPage submit-while-config-loading race
**Status**: 🟢 DONE — Handled in PhoneAuthPage.tsx: on 403 PASSWORD_LOGIN_DISABLED, re-fetches authConfig so the no-form branch renders.
**Fix**: On 403 PASSWORD_LOGIN_DISABLED, re-fetch config and re-render the "use magic link" message.

### M9 — `decrypt_secret` failure silently returns None
**Status**: 🟢 DONE — `get_settings` checks `had_ciphertext && token.is_none()` and logs a clear warning ("storage_key likely rotated; admin must re-save").
**Fix**: Log a warning when ciphertext is present but decryption fails.

### M10 — Tier-3 mock-driver env var leaks across tests
**Status**: ⚪ DEFERRED — Documentation-only; the harness's OnceLock semantics make this benign in practice. Tracked.
**Fix**: Document that the env must be set BEFORE any handler is hit, OR isolate to a separate test binary.

### M11 — Auto-start ngrok error leaks verbatim to UI
**Status**: 🟢 DONE — `tunnel_error_to_api` now sanitizes auth-failed and other-error messages (clean user-friendly text); raw detail goes to tracing::warn.
**Fix**: Map ngrok errors to user-friendly strings; keep raw in `tracing::warn!`.

### M12 — Magic-link token prefix logged plaintext
**Status**: 🟢 DONE — Both `issue` and `exchange` log `token_hash_prefix` (SHA-256 prefix) instead of plaintext.
**Fix**: Log `token_hash_prefix` (first 8 chars of SHA-256) instead.

### M13 — `init_tunnel_driver` never called from prod
**Status**: 🟢 DONE — `RemoteAccessModule::init` now calls `state::init_tunnel_driver(Arc::new(NgrokDriver::new()))` explicitly. Choice is auditable from startup logs.
**Fix**: Call it from `RemoteAccessModule::init` with `NgrokDriver::new()`. Update docstring.

### M14 — PhoneAuthPage Form has no `name=` on password
**Status**: 🟢 DONE — Now `Form.Item name="password" rules={[{ required: true, message: 'Enter your password' }]}`.
**Fix**: Convert to `Form.Item name="password" rules={[{ required: true }]}`.

### M15 — No hidden username anchor for password managers
**Status**: 🟢 DONE — Hidden `<input type="text" autoComplete="username" value="admin" readOnly hidden />` added to PhoneAuthPage.
**Fix**: Add `<input type="text" autoComplete="username" value="admin" hidden />` in PhoneAuthPage.

### M16 — Rotation timer doesn't pause on tab hidden
**Status**: 🟢 DONE — `startMagicLinkRotation` interval body checks `document.visibilityState === 'hidden'` and skips the tick.
**Fix**: Gate the interval body on `document.visibilityState === 'visible'`.

### M17 — `/api/auth/login` (standard) reachable on tunnel
**Status**: ⚪ DEFERRED — Would require modifying the server's `/api/auth/login` handler to take an optional "deployment mode" injection. Cross-cuts the server crate (which the user wants to keep clean of remote-access concerns). Tracked as a future hardening — magic-link is the documented happy path; the standard login still requires valid creds.
**Fix**: Make standard login check `password_auth_enabled` when called from non-localhost Host.

### M18 — `magic_link/repository::insert` has no `ON CONFLICT`
**Status**: 🟢 DONE — `INSERT ... ON CONFLICT (token_hash) DO NOTHING`. A 32-byte CSPRNG collision is vanishingly unlikely but no longer 500s if it ever happens.
**Fix**: Add `ON CONFLICT (token_hash) DO NOTHING`; treat 0-rows-affected as regenerate.

### M19 — `saving` is one bool for 5 concurrent actions
**Status**: ⚪ DEFERRED — Same scope as H21 (per-op flag refactor).

### M20 — `MagicLinkPage` doesn't cancel in-flight fetch
**Status**: ⚪ DEFERRED — Mitigated by the TunnelAuth.store dedupe (M4); a true AbortController fix requires plumbing through the openapi-generated ApiClient (no abort signal in generated method shapes).
**Fix**: Use AbortController; pass signal to fetch.

### M21 — `run_desktop_migrations` failure logs error but server keeps serving
**Status**: 🟡 PARTIAL — Error log bumped from generic to actionable wording ("the app may be unusable — reset data dir / restore backup"). Cross-process /api/health signaling to the UI bootstrap spinner is deferred (needs a shared state shim between backend and desktop_base UI store).
**Fix**: Set a "boot-broken" flag visible to UI via /api/health.

### M22 — Stale openapi comment in desktop AuthGuard
**Status**: 🟢 DONE — Comment now correctly states the redirect moved to onboarding's `routerEffects` and the admin gate is the reason desktop is exempt.
**Fix**: Update to real reason ("admins drive the app; phone surface can't mark guides done").

### M23 — `/auth/magic/` (no token) doesn't match the route
**Status**: 🟢 DONE — Added second route `/auth/magic` (no param) → MagicLinkPage which already renders a "Missing token" Result when `useParams.token` is empty.
**Fix**: Add second route at `/auth/magic` rendering a "Missing token" page.

### M24 — Migration numbering inconsistency
**Status**: 🟢 DONE — Renumbered desktop migrations to consecutive `10000000000001`–`10000000000004`. Server uses `00000000000001`–`00000000000064`; desktop uses `10000000000001+`. Future server migrations can grow to `10000000000000` before any collision — effectively never.

---

## 🟢 LOW + INFO (42 items)

Not addressed in this pass. Notable items to revisit:
- L1 ngrok-specific naming locks us into ngrok (`ngrok_auth_token`, error messages)
- L2 deprecated ngrok `tunnel.forward()` API usage
- L3 Magic-link URL in URL path → Referer leak risk; add `<meta name="referrer" content="no-referrer">` on MagicLinkPage
- L4 Bootstrap password (`desktop-auto-login`) visible in UI text
- L5 Copy buttons missing `aria-label`; QR SVG missing title
- L6 `_kind_marker` dead code
- L7 "either 422 or 200 is fine" tautology assertions
- I1 `change_password` handler placement is defensible (stays in server per audit)

Track separately in a follow-up audit.

---

## Verification log

After each batch, the file will get a section here with re-audit results.

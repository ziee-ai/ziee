# Chunk BG — DRIFT scan (round 1)

Drift = any place the de-globalization could diverge from the pre-refactor
behaviour/surface. Each candidate reconciled below.

- **DRIFT-1.1** — verdict: none. `Repos.X` → `ctx.X()` builds a fresh repo per
  call instead of Deref-ing the cached global. Repos are `{ pool }` wrappers with
  no mutable state; the pool is the identical `Arc<PgPool>` (layered from the
  same `pool` the factory init'd with). Behaviour identical. Confirmed by E8
  golden + integration-test-target compile (both green).

- **DRIFT-1.2** — verdict: none. Handler signatures gained `Extension<AuthContext>`
  (and some jwt/callback handlers keep their existing extractors). `Extension`
  is non-body-consuming and order-independent in axum, so extractor ordering is
  irrelevant; no route/DTO/permission changed → openapi.json + types.ts
  byte/canonical-identical on BOTH surfaces (E8).

- **DRIFT-1.3** — verdict: none. Event emits moved from
  `event_bus.emit_async(UserEvent::created(u))` to
  `ctx.events.emit_user(UserEvent::Created{user:u})`. The app sink wraps into the
  SAME `AppEvent::User` variant and calls the same `emit_async`; the dead
  `AppEvent`-wrapping ctors + the `From<AuthProviderEvent>` bridge were removed
  ONLY after confirming zero live callers remained (grep). Same subscribers, same
  fire-and-forget.

- **DRIFT-1.4** — verdict: none. `mint_session_tokens`/`session_expiries` gained a
  leading `&PgPool`. All in-crate callers pass `ctx.pool()`; the three desktop
  callers + `app/handlers.rs` pass `Repos.pool()`/`ziee::Repos.pool()` (the same
  global pool, and those files are outside the gate scope). The mint→register
  two-step + admin-configurable-lifetime fallback logic is untouched.

- **DRIFT-1.5** — verdict: none. `JwtSettings` is field-for-field identical to
  `JwtConfig`; `try_new(impl Into)` + the app-side `From` keep every call site
  (incl. `ziee::JwtConfig` tests + desktop `JwtService::new`) compiling. The
  weak-secret refusal, banned-list, leeway, and debug-seconds seam are unchanged
  (jwt.rs unit tests green).

- **DRIFT-1.6** — verdict: none. `url_validator` → `core::outbound` is a pure
  re-export; the providers keep their exact `cfg!(debug_assertions)` policy branch
  + redirect-disabled fallback. SSRF confinement unchanged.

- **DRIFT-1.7** — verdict: none. `secret_key` threaded param equals the boot-time
  copy of the global `storage_key()`; the dual-column compat branch is identical.

- **DRIFT-1.8** — verdict: none. `oauth2` now stores the pool (was `_pool`)
  solely to build `AuthRepository` for the same oauth-session rows; no new query
  or data path.

- **DRIFT-1.9** — verdict: none (extension coverage). `Extension<AuthContext>` is
  layered in BOTH router-build sites (`lib.rs::setup_server` — used by tests +
  desktop — and `main.rs`), so no handler 500s on a missing extension. Verified:
  `cargo check --test integration_tests` exit 0.

**Unresolved drifts: 0**

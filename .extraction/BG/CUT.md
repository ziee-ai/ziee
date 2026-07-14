# Chunk BG — de-globalize the auth-module app-globals (CUT manifest)

**This is an IN-ZIEE REFACTOR, not a crate move.** No code leaves the `ziee`
crate; the SDK submodule is untouched. The goal is to sever the auth module
(`modules/auth/*`) and the co-located user repos/handlers (`modules/user/*`)
from the six app-global singletons the Chunk-BA STOP_REPORT identified as
blocking a future `ziee-auth` crate, so BA-full can proceed. Everything is
**equivalence-preserving** (behaviour byte-identical; no wire change — E8 golden
identical on BOTH surfaces).

## The six seams (STOP_REPORT blockers) → how each is now injected

| # | App-global removed from auth/user | Injection mechanism |
|---|---|---|
| 1 | `crate::core::Repos` (global repo aggregator) | a per-request `AuthContext` handle carrying an `Arc<PgPool>`; repos built per-call from it (`ctx.auth()/user()/group()/session_settings()/pool()`). Repo-layer fns (`refresh_tokens`, `session_settings`) take a `&PgPool` param. |
| 2 | `crate::modules::sync::publish[_session_to_users]` | injected `AuthSyncSink` trait (`ctx.sync`), app wires it to the real `sync::publish`. |
| 3 | `crate::core::secrets::storage_key()` | threaded `storage_key: Option<&str>` param into `providers::repository::{prepare_config_for_write,create_provider,update_provider}`; handlers supply `ctx.secret_key()` (copied app-side from the global at boot). |
| 4 | `crate::core::AppEvent` / `core::events::EventBus` | injected `AuthEventSink` trait (`ctx.events`, extends the B2/B5 erased-event seam — the app impl wraps module events into `AppEvent` and fires the real `EventBus`). Dead `AppEvent`-wrapping constructors removed from `events.rs`. |
| 5 | `crate::utils::url_validator::*` (SSRF) | re-homed behind an app-side `crate::core::outbound` adapter (see TRANSFORMS Decision 5); providers name `core::outbound`, not `url_validator`. |
| 6 | `crate::core::config::JwtConfig` | auth-owned `JwtSettings` struct in `jwt.rs`; app-side `From<JwtConfig>` (in `core/config.rs`) bridges it, so every `try_new(config.jwt)` call site is unchanged. |

## Files — NEW (2)

- new: `src-app/server/src/modules/auth/context.rs` — declares `AuthContext`
  handle + `AuthEventSink` / `AuthSyncSink` traits (the consumer-owned
  abstractions the app installs).
- new: `src-app/server/src/core/outbound.rs` — app-side re-export adapter for
  the generic `url_validator` helpers (seam 5).

## Files — MODIFIED (24)

### App-side wiring (installs the injected impls — the only place the globals are still named for auth)
- `src-app/server/src/core/events.rs` — `EventBusAuthSink` / `PublishSyncSink` impls + `build_auth_context(pool, event_bus)`.
- `src-app/server/src/core/config.rs` — `impl From<JwtConfig> for JwtSettings` (in the shared `core` tree so BOTH the lib and the `ziee` bin see it).
- `src-app/server/src/core/mod.rs` — `pub mod outbound;`.
- `src-app/server/src/lib.rs` — layers `Extension<AuthContext>`; `pub use …JwtSettings`.
- `src-app/server/src/main.rs` — layers `Extension<AuthContext>` (bin re-compiles `modules/`, so it needs its own layer + the shared `From` impl).

### Auth module (de-globalized)
- `modules/auth/mod.rs` — `pub mod context;`.
- `modules/auth/jwt.rs` — `JwtSettings`; `try_new/new` take `impl Into<JwtSettings>`.
- `modules/auth/refresh_tokens.rs` — `session_expiries(pool,…)` / `mint_session_tokens(pool,…)`.
- `modules/auth/session_settings.rs` — handlers pull `AuthContext`; `ctx.session_settings()` + `ctx.sync.publish`.
- `modules/auth/events.rs` — dead `AppEvent`-wrapping ctors removed.
- `modules/auth/handlers.rs` — every handler pulls `AuthContext`; `Repos.*`→`ctx.*`, `EventBus`→`ctx.events`, `sync_publish`→`ctx.sync.publish`; private helpers thread `ctx`/`pool`.
- `modules/auth/providers/{repository,health,local,apple,oauth2,events}.rs` — secrets param, event sink, `AuthRepository`/`UserRepository`-from-pool, `core::outbound`, comment reword.

### User module (co-located, same handle)
- `modules/user/events.rs` — dead `AppEvent`-wrapping ctors removed.
- `modules/user/handlers/{user,groups}.rs` — pull `AuthContext`; `Repos.*`→`ctx.*` (incl. `Repos.skill`/`Repos.file` → repos-from-pool), event sink, sync sink.

### Non-auth callers of the re-signatured repo fns (pass a pool; not gate-scoped)
- `modules/app/handlers.rs` (`Repos.pool()`), desktop `modules/{auth,magic_link,tunnel_auth}` (`ziee::Repos.pool()`).

### Tests (signature follow-through)
- `tests/auth/mod.rs`, `tests/auth/profile_self_service_test.rs` — `ensure_unique_username(pool, base)`.

# Chunk BG — TRANSFORMS (non-byte-identical changes + rationale)

Every seam that changed a signature or moved a naming site is recorded here with
its design decision + resolution. Zero `TBD` / `TODO` / `ASK`. All transforms are
equivalence-preserving; the E8 golden (types.ts byte-identical + openapi.json
canonical, BOTH surfaces) is the machine proof that no wire surface moved.

---

## Decision 1 — `Repos` global → `AuthContext` handle (repo layer + handlers)

The auth/user handlers reached `crate::core::Repos` (a `pub const` ZST global
Deref-ing to a lazily-init'd `RepositoryFactory`). The STOP_REPORT scoped the
blocker to the repo layer (`refresh_tokens`, `session_settings`), but the gate
requires the whole auth+user surface clean, and BA moves the handlers too — so
handlers are de-globalized as well.

**Resolution:** A per-request `AuthContext` (new `context.rs`) carries an
`Arc<PgPool>` and exposes `pool()` + `auth()/user()/group()/session_settings()`
that build a fresh repo from the pool. Repos are stateless pool wrappers, so
per-call construction is behaviourally identical to the cached global accessor.
Handlers receive `Extension<AuthContext>` (layered once at boot in
`lib.rs`/`main.rs`); `Repos.pool()`→`ctx.pool()`, `Repos.auth`→`ctx.auth()`, etc.
Cross-module `Repos.skill`/`Repos.file` (in `delete_user`) become
`SkillRepository::new(ctx.pool().clone())` / `FileRepository::new(…)`. The
repo-layer fns `session_expiries`/`mint_session_tokens` take an explicit
`&PgPool` (their non-auth callers pass `Repos.pool()` / `ziee::Repos.pool()` —
those files are app/desktop, outside the gate scope).

## Decision 2 — `sync::publish` → `AuthSyncSink`

`session_settings.rs` + user handlers called the global `sync::publish` /
`publish_session_to_users`.

**Resolution:** An `AuthSyncSink` trait (`ctx.sync`) with `publish(entity, action,
id, audience, origin)` + `publish_session_to_users`. The app installs
`PublishSyncSink` (in `core/events.rs`) that forwards to the real functions. The
sink signature still uses the app's `SyncEntity`/`Audience` value types (NOT
gate-flagged, and made app-extensible only in Chunk B5); BG's job is to remove
the direct call to the global publish FUNCTION, which it does.

## Decision 3 — `secrets::storage_key()` → threaded param

`providers::repository::prepare_config_for_write` read the process-global
`storage_key()` inline.

**Resolution:** Thread `storage_key: Option<&str>` through
`prepare_config_for_write` → `create_provider`/`update_provider`; the admin
handlers pass `ctx.secret_key()`. `AuthContext` copies the key from
`crate::core::secrets::storage_key()` ONCE at boot (`build_auth_context`, app
side). Same value, same compat-mode branch — behaviour identical.

## Decision 4 — `AppEvent`/`EventBus` → `AuthEventSink` (extends B2/B5 erased seam)

Handlers took `Extension<Arc<EventBus>>` and emitted
`event_bus.emit_async(UserEvent::created(u))` / `AuthProviderEvent::x().into()`;
`events.rs` (auth+user) carried dead `AppEvent`-wrapping constructors; the health
module took `&EventBus`.

**Resolution:** An `AuthEventSink` trait (`ctx.events`) with `emit_user(UserEvent)`
/ `emit_auth_provider(AuthProviderEvent)`. Callers build the RAW module-event
variant; the app-installed `EventBusAuthSink` (in `core/events.rs`) performs the
`AppEvent::User(..)` / `AppEvent::AuthProvider(..)` wrapping and fires the real
`EventBus` — so the app still owns `AppEvent`, and the same subscribers still
receive the same events. This EXTENDS the existing B2 erased-`EventHandler` seam
(events still flow to the bus type-erased) rather than inventing a parallel bus.
The dead `impl AuthEvent`/`impl UserEvent` `AppEvent` constructors + the
`From<AuthProviderEvent> for AppEvent` bridge were deleted (no live callers after
the emit sites moved to the sink). `health.rs` takes `pool: &PgPool` + `events:
&dyn AuthEventSink`.

## Decision 5 — `url_validator` (SSRF) → `core::outbound` adapter (reported "your call")

The task offered: pass the validator in, OR move the generic `url_validator` to
`ziee-framework`. **Chosen: re-home behind an app-side `crate::core::outbound`
adapter, NOT trait-injection.** Rationale: (a) the SDK move is impossible in this
in-ziee chunk (submodule off-limits); (b) trait-injection into `oauth2.rs` is a
risky, non-equivalence-trivial restructure — its outbound path is a process-global
`OnceLock<reqwest::Client>` fed to `openidconnect` as fn-pointers
(`async_http_client`), so there is no per-request handle to thread a trait
through; (c) `url_validator` is domain-free framework infra (a pure URL/IP
allowlist + validated-client builder), fundamentally different from the app
singletons the other seams remove. `core/outbound.rs` re-exports
`{OutboundUrlPolicy, build_validated_client, validate_outbound_url}`;
`providers/{apple,oauth2}.rs` name `crate::core::outbound::…`. Behaviour is
byte-identical (same functions, same `cfg!(debug_assertions)` DEV_LOCAL/PUBLIC
policy). When `url_validator` eventually lands in `ziee-framework`, `ziee-auth`
retargets this one import.

## Decision 6 — `core::config::JwtConfig` → auth-owned `JwtSettings`

`jwt.rs` named `crate::core::config::JwtConfig` (a `ziee-core` type re-exported
through the app config). Even naming it via `ziee_core::config::JwtConfig` would
still match the gate pattern `core::config::JwtConfig`.

**Resolution:** An auth-owned `JwtSettings` struct (field-for-field identical) in
`jwt.rs`; `JwtService` holds it; `try_new`/`new` take `impl Into<JwtSettings>`.
The bridging `impl From<JwtConfig> for JwtSettings` lives in `core/config.rs`
(the shared `core` tree compiled by BOTH the lib and the `ziee` bin — NOT `lib.rs`,
which the bin doesn't include). Because `try_new` takes `impl Into`, every
existing `JwtService::try_new(config.jwt.clone())` call site AND the cross-crate
tests passing `ziee::JwtConfig` compile unchanged. Pure field move.

## Decision 7 — `ensure_unique_username` test-visible signature

`ensure_unique_username` is `#[doc(hidden)] pub` (called by integration tests).
De-globalizing its `Repos.user` forced a `pool` param.

**Resolution:** `ensure_unique_username(pool: &PgPool, base: &str)`; the two test
callers (`tests/auth/mod.rs`, `profile_self_service_test.rs`) pass their existing
test pool. This is the only test-facing signature change; enumerated in
TESTS-MOVED.

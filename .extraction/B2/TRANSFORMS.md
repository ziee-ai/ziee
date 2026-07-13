# Chunk B2 — TRANSFORMS (every non-byte-identical change + rationale)

The moved code is byte-identical to its pre-extraction ziee form EXCEPT the
transforms below. `create_modules`/`initialize_modules`/`build_api_router`,
`ModuleEntry`, `MODULE_ENTRIES`, and every relocated config sub-type carry
**verbatim** bodies; the CORS/rate-limit fns are verbatim except their config
parameter type (T-4).

- **T-1** `ServerConfig` (`ziee-core::config`): the B1 placeholder `struct
  ServerConfig {}` is fleshed out to `{ postgresql, server: HttpServerConfig,
  logging: Option<LoggingConfig>, jwt: JwtConfig }`, plus the `database_url()` /
  `server_address()` methods moved off `Config`. — **why:** this is the Config
  split. The four grouped fields + two pure methods are copied verbatim from ziee's
  `Config`; grouping them into `ServerConfig` is what lets `ModuleContext` carry the
  app-agnostic config.

- **T-2** `HttpServerConfig` (`ziee-core::config`): ziee's former `ServerConfig`
  (host / port / api_prefix / cors / rate_limit / trust_forwarded_headers /
  max_file_upload_mb) is renamed `HttpServerConfig`. — **why:** name collision — the
  grouping type must own the `ServerConfig` name (task + the B1 stub). The struct
  body + all serde defaults are verbatim; only the type identifier changed. No ziee
  code names this type outside `config.rs` (0 external `config::ServerConfig`
  references), and it is re-exported as `HttpServerConfig`, so nothing breaks.

- **T-3** `Config` (ziee `core/config.rs`): the four server fields become
  `#[serde(flatten)] pub server_config: ServerConfig`; `Config` gains `Deref`/
  `DerefMut` targeting `ServerConfig`; `database_url`/`server_address` are removed
  from `Config` (now reached via `Deref`). — **why:** compose the split. `flatten`
  keeps the wire shape byte-identical (the `postgresql`/`server`/`jwt`/`logging`
  keys stay top-level); `Deref`/`DerefMut` keep every `config.postgresql` /
  `config.server.port = …` / `config.database_url()` call site (server + desktop)
  unchanged via field/method auto-deref. `load_from`/`resolve_paths`/the domain
  sub-configs stay verbatim. No `Config {…}` struct-literal exists anywhere, so the
  new `server_config` field breaks no construction site.

- **T-4** `create_cors_layer` / `apply_rate_limit_layer` (`ziee-framework::
  app_builder`): parameter type `&Config` → `&ServerConfig`; bodies read
  `config.server.cors` / `config.server.rate_limit` unchanged. — **why:** the
  framework cannot name ziee's `Config`; both fns only ever touched `config.server`,
  which is on `ServerConfig`. ziee call sites (`main.rs`, `lib.rs`) pass `&config`
  (a `Config`), which **deref-coerces** to `&ServerConfig`, so they are byte-for-byte
  unchanged.

- **T-5** `ModuleContext` (`ziee-framework::module_api`): `config: Arc<Config>` →
  `config: Arc<ServerConfig>` + a new `app_config: Arc<dyn Any + Send + Sync>` field
  (+ 3-arg `new`). — **why:** the design gate — the framework context must be
  domain-free. The typed field carries `ServerConfig` (satisfying all 34
  `ctx.config.server` / `ctx.config.jwt` sites unchanged, since `ServerConfig` owns
  `server` + `jwt`); the app injects its full `Config` through the opaque
  `app_config` slot. See `## Decision`.

- **T-6** module `init` domain-config reads (6 sites: voice, server_update,
  code_sandbox, bio_mcp, lit_search, web_search, control_mcp, js_tool, project,
  chat, scheduler): `ctx.config.<domain>` / `ctx.config.clone()` →
  `crate::module_api::app_config(ctx).<domain>` / `app_config(ctx)`. — **why:** those
  domain sub-configs (and the full-`Config` clones threaded into the chat/project
  extension registries + scheduler) are NO LONGER on `ctx.config` (now
  `ServerConfig`). `app_config(ctx)` recovers `Arc<Config>` from the injected opaque
  slot; the read is value-identical (same field, same `Arc<Config>`).

- **T-7** `EventHandler` (`ziee-framework::events`): trait moved from ziee
  `core/events.rs`; `handle`'s event parameter `&AppEvent` → `&(dyn Any + Send +
  Sync)`. — **why:** `AppModule::register_event_handlers` returns `Vec<Arc<dyn
  EventHandler>>`, so the trait must move with `AppModule`; but `AppEvent` is
  domain-coupled (its variants wrap ziee module events) and stays app-side. Erasing
  the event to `&dyn Any` keeps the framework `EventHandler` domain-free. The event
  is never on the OpenAPI wire, so this is observably equivalent.

- **T-8** `EventBus` (ziee `core/events.rs`, retained) + the 3 concrete handlers
  (hub / mcp / assistant `event_handlers.rs`): `EventBus::emit`/`emit_async` pass
  `&event as &(dyn Any + Send + Sync)`; each handler's `handle` takes `&(dyn Any +
  Send + Sync)` and prepends `let Some(event) = event.downcast_ref::<AppEvent>()
  else { return Ok(()) };`. — **why:** the app side of the T-7 erasure. `EventBus`
  keeps `AppEvent` + all dispatch/semaphore logic verbatim; the coercion is
  automatic (`AppEvent: 'static + Send + Sync` — already required by the pre-existing
  `emit_async` spawn). The handlers' `match event { … }` bodies are byte-identical
  after the one-line downcast.

- **T-8b** desktop crate (`src-app/desktop/tauri`): the two desktop `EventHandler`
  impls (`llm_provider`/`mcp` `event_handlers.rs`) get the same `&dyn Any` +
  `downcast_ref::<AppEvent>()` treatment as T-8; the desktop openapi-gen
  `ServerContext::new` site (`openapi.rs`) passes the ServerConfig + `Arc<Config>`
  opaque-slot args (T-5). — **why:** `ziee-desktop` embeds the server crate and
  re-uses `ziee::{EventHandler, ServerContext}`, so it must track the same signature
  changes. `cargo check -p ziee-desktop` is green after these edits.

- **T-9** `module_api/mod.rs` + `core/app_builder.rs` shims: the moved definitions
  are replaced by `pub use ziee_framework::…`; `app_builder.rs` retains
  `register_event_handlers` + the two module-registration tests; `mod.rs` adds the
  `app_config(ctx)` helper. — **why:** decision N2 — every `crate::module_api::{…}`
  + `#[distributed_slice(MODULE_ENTRIES)]` site and every `core::app_builder::…`
  caller resolves unchanged.

## Decision

**Question:** How can `ModuleContext` carry only `ServerConfig` when 11 module
`init`s (and the chat/project extension registries + scheduler) need domain
sub-configs or the whole `Config`, AND how can the framework `AppModule` move when
its `register_event_handlers` returns a domain-coupled `EventHandler`?

**Resolution.** Two coupled erasures, both equivalence-preserving:

1. **Config — opaque `app_config` slot (T-5/T-6).** `ModuleContext` carries the
   typed `Arc<ServerConfig>` (which satisfies all `ctx.config.server`/`.jwt` sites
   directly, since `ServerConfig` owns those) PLUS an `app_config: Arc<dyn Any +
   Send + Sync>` into which ziee injects `Arc<Config>`. Modules recover the full
   `Config` via `module_api::app_config(ctx)` (a downcast that never fails — ziee
   always injects `Config`). This is dependency injection, not a global: it keeps
   the framework `ModuleContext` free of any app config type while every domain read
   returns the identical `Arc<Config>`. Alternatives rejected: (a) `ModuleContext`
   holding `Config` — violates the gate; (b) a ziee `OnceLock<Config>` global —
   against the plan's later de-globalize direction (BG). The linkme-forced
   constraint (the `MODULE_ENTRIES` slice element type `ModuleEntry` must be
   concrete, so `AppModule` and its return types must be domain-free) is why the
   framework side cannot simply name `Config`.

2. **Events — `&dyn Any` erasure (T-7/T-8).** The same linkme constraint forces
   `AppModule::register_event_handlers -> Vec<Arc<dyn EventHandler>>` to reference a
   domain-free `EventHandler`. `EventHandler` therefore moves to the framework with
   its event parameter erased to `&(dyn Any + Send + Sync)`; `AppEvent` + `EventBus`
   stay app-side (they are the B5-scoped, domain-coupled half). `EventBus` dispatch
   coerces `&AppEvent` to the erased type and the 3 handlers `downcast_ref` back —
   value-identical for the real event, and the whole event system is off the OpenAPI
   wire, so E8 is untouched. Moving only the trait (not `EventBus`/`AppEvent`) keeps
   B2 minimal and leaves B5's EventBus genericization intact.

Both erasures are proven equivalence-preserving by the golden gate: `types.ts`
byte-identical, `openapi.json` canonically-equal (config + events never reach the
spec). Zero unresolved markers remain; every transform carries a rationale.

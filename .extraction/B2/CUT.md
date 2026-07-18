# Chunk B2 — module system + Config split (CUT manifest)

Move the module system (`AppModule` / `ModuleContext` / `ModuleEntry` /
`MODULE_ENTRIES`) and `app_builder` (module discovery, router assembly, CORS +
rate-limit) into `sdk/crates/ziee-framework` (depends on `ziee-core` +
`ziee-identity`), consumed by ziee via equivalence-preserving re-export shims
(decision N2). Flesh out `ServerConfig` in `ziee-core` and split ziee's
monolithic `Config` so `ModuleContext` carries only `ServerConfig`.

## Files

Per decision **N2**, ziee's `module_api/mod.rs`, `core/app_builder.rs`,
`core/config.rs`, `core/events.rs` are RETAINED as re-export shims (the moved
*definitions* are deleted). `module_api/backend_module.rs` and
`module_api/types.rs` had NO retained content and are deleted outright (whole-file
`E6 source-absent` applies to those two).

- move: `src-app/server/src/module_api/backend_module.rs` → `sdk/crates/ziee-framework/src/module_api.rs`
- move: `src-app/server/src/module_api/types.rs` → `sdk/crates/ziee-framework/src/module_api.rs`
- move: `src-app/server/src/core/app_builder.rs` → `sdk/crates/ziee-framework/src/app_builder.rs`

Config sub-types relocated into the fleshed-out `ServerConfig` (from ziee's
`core/config.rs`, RETAINED as a shim + composing `Config`):

- move: `src-app/server/src/core/config.rs` → `sdk/crates/ziee-core/src/config.rs`

The domain-free `EventHandler` trait extracted from ziee's `core/events.rs`
(RETAINED, keeps `AppEvent` + `EventBus`):

- move: `src-app/server/src/core/events.rs` → `sdk/crates/ziee-framework/src/events.rs`

## Symbols

Module system (byte-preserved bodies into `ziee-framework::module_api`):
- symbol: `AppModule` (sdk/crates/ziee-framework/src/module_api.rs)
- symbol: `ModuleContext` (sdk/crates/ziee-framework/src/module_api.rs)
- symbol: `ModuleEntry` (sdk/crates/ziee-framework/src/module_api.rs)
- symbol: `MODULE_ENTRIES` (sdk/crates/ziee-framework/src/module_api.rs)

App builder (byte-preserved bodies into `ziee-framework::app_builder`):
- symbol: `create_modules` (sdk/crates/ziee-framework/src/app_builder.rs)
- symbol: `initialize_modules` (sdk/crates/ziee-framework/src/app_builder.rs)
- symbol: `build_api_router` (sdk/crates/ziee-framework/src/app_builder.rs)
- symbol: `create_cors_layer` (sdk/crates/ziee-framework/src/app_builder.rs)
- symbol: `apply_rate_limit_layer` (sdk/crates/ziee-framework/src/app_builder.rs)

Config split (into `ziee-core::config`):
- symbol: `ServerConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `HttpServerConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `PostgreSqlConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `EmbeddedPostgreSqlConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `ExternalPostgreSqlConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `PoolConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `LoggingConfigPostgres` (sdk/crates/ziee-core/src/config.rs)
- symbol: `CorsConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `RateLimitConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `JwtConfig` (sdk/crates/ziee-core/src/config.rs)
- symbol: `LoggingConfig` (sdk/crates/ziee-core/src/config.rs)

Event trait (into `ziee-framework::events`):
- symbol: `EventHandler` (sdk/crates/ziee-framework/src/events.rs)

## Symbols that STAY in ziee (domain-coupled — never moved)

- `Config` (`core/config.rs`) — COMPOSES `ServerConfig` via `#[serde(flatten)]` + `Deref`/`DerefMut`; owns the domain sub-configs + `load_from`/`resolve_paths`.
- domain sub-configs `AppConfig`, `CodeSandboxConfig`, `BioMcpConfig`, `LitSearchConfig`, `WebSearchConfig`, `VoiceConfig`, `ControlMcpConfig`, `JsToolConfig`, `SecretsConfig`, `CachesConfig`, `UpdateCheckConfig` (`core/config.rs`).
- `AppEvent` enum + `EventBus` dispatcher (`core/events.rs`) — every `AppEvent` variant wraps a ziee module event; moves in B5.
- `register_event_handlers` (`core/app_builder.rs`) — constructs the domain-coupled `EventBus`.

## Shims (retained ziee files — decision N2)

- `src-app/server/src/module_api/mod.rs` → `pub use ziee_framework::{AppModule, ModuleContext, ModuleEntry, MODULE_ENTRIES};` + a new `app_config(ctx)` helper that recovers ziee's `Config` from the opaque `ModuleContext.app_config` slot.
- `src-app/server/src/core/app_builder.rs` → `pub use ziee_framework::app_builder::{create_modules, initialize_modules, build_api_router, create_cors_layer, apply_rate_limit_layer};` + retains `register_event_handlers` + the two module-registration tests.
- `src-app/server/src/core/config.rs` → `pub use ziee_core::config::{ServerConfig, HttpServerConfig, PostgreSqlConfig, …, JwtConfig, LoggingConfig};`; `Config` composes `ServerConfig` (flatten + Deref).
- `src-app/server/src/core/events.rs` → `pub use ziee_framework::EventHandler;`; retains `AppEvent` + `EventBus` (dispatch erases `&AppEvent` → `&dyn Any`).
- `src-app/server/Cargo.toml` → adds `ziee-framework = { path = "../../sdk/crates/ziee-framework" }`.

## Design-gate

**Config split.** `ModuleContext` must carry only the app-agnostic
`ServerConfig` (postgresql / server / logging / jwt) — never the app's monolithic
`Config`. `ServerConfig` is fleshed out in `ziee-core`; ziee's `Config` COMPOSES
it via `#[serde(flatten)]` so the serialized (YAML) shape is byte-identical, with
`Deref`/`DerefMut` so every `config.postgresql` / `config.server` / `config.jwt` /
`config.database_url()` call site is unchanged. Modules that need a domain
sub-config read the full `Config` from the opaque `ModuleContext.app_config` slot
(ziee injects `Arc<Config>`; modules downcast via `module_api::app_config`). No
config type is on the OpenAPI surface (`Config` and its sub-types are
`Deserialize`-only — 0 occurrences of any config type in the baseline
`openapi.json`/`types.ts` beyond the unrelated `McpServerConfig`), so the split is
E8-neutral. The linkme `MODULE_ENTRIES` slice is DEFINED in `ziee-framework` and
registered into from ziee's 44 module sites via the `crate::module_api`
re-export — proven to link (the full-router regen emits every module's routes).

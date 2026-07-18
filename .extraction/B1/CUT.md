# Chunk B1 — `ziee-core` foundation (CUT manifest)

Mechanical MOVE of ziee's foundation types into `sdk/crates/ziee-core`, consumed
by ziee via equivalence-preserving re-export shims (decision N2), so its ~323
`crate::common::AppError` call sites stay byte-for-byte unchanged.

## Files

Content-level relocations (source → SDK dest). NOTE: per decision **N2**, each
ziee source file is RETAINED as a pure re-export shim (the moved *definitions*
are deleted, not the file), so the classic whole-file `E6 source-absent` check is
intentionally waived for this chunk — see `## Shims` below and `DRIFT-1.5`.

- move: `src-app/server/src/common/type.rs` → `sdk/crates/ziee-core/src/error.rs`
- move: `src-app/server/src/common/macros.rs` → `sdk/crates/ziee-core/src/macros.rs`
- move: `src-app/server/src/core/app_state.rs` → `sdk/crates/ziee-core/src/app_state.rs`

New foundation type introduced in the SDK (no ziee source):

- move: `sdk/crates/ziee-core/src/config.rs` → `sdk/crates/ziee-core/src/config.rs`

## Symbols

Error surface (byte-preserved into `ziee-core/src/error.rs`):
- symbol: `ApiResult` (sdk/crates/ziee-core/src/error.rs)
- symbol: `ApiError` (sdk/crates/ziee-core/src/error.rs)
- symbol: `AppError` (sdk/crates/ziee-core/src/error.rs)

Core macros (into `ziee-core/src/macros.rs`):
- symbol: `pascal_to_camel_case` (sdk/crates/ziee-core/src/macros.rs)
- symbol: `sse_event_enum` (sdk/crates/ziee-core/src/macros.rs)
- symbol: `impl_string_to_enum` (sdk/crates/ziee-core/src/macros.rs)
- symbol: `impl_json_from` (sdk/crates/ziee-core/src/macros.rs)

Base app-state globals (into `ziee-core/src/app_state.rs`):
- symbol: `APP_DATA_DIR` (sdk/crates/ziee-core/src/app_state.rs)
- symbol: `set_app_data_dir` (sdk/crates/ziee-core/src/app_state.rs)
- symbol: `get_app_data_dir` (sdk/crates/ziee-core/src/app_state.rs)
- symbol: `SERVER_ADDR` (sdk/crates/ziee-core/src/app_state.rs)
- symbol: `set_server_addr` (sdk/crates/ziee-core/src/app_state.rs)
- symbol: `get_server_addr` (sdk/crates/ziee-core/src/app_state.rs)
- symbol: `set_app_name` (sdk/crates/ziee-core/src/app_state.rs)

Placeholder config:
- symbol: `ServerConfig` (sdk/crates/ziee-core/src/config.rs)

## Symbols that STAY in ziee (not moved — coupled to ziee-only paths)

- `make_transparent!`, `impl_json_option_from!` (reference `crate::common::types::JsonOption`)
- `define_extension_content!` (references chat `MessageContentData`)
- `PaginationQuery`, `PAGINATION_MAX_PER_PAGE`, `DEFAULT_PAGE_SIZE` (not in B1 scope; `PaginationQuery` derives `JsonSchema` → OpenAPI-facing)
- `CACHES_CONFIG`/`get/set_caches_config` (depend on `CachesConfig`, which moves in B2)
- `MAX_FILE_UPLOAD_BYTES` + accessors + `UPLOAD_BODY_LIMIT_SLACK_BYTES` (keep the docker/nginx tests + private slack const app-side)

## Shims (retained ziee re-export files — decision N2)

- `src-app/server/src/common/type.rs` → `pub use ziee_core::{ApiResult, AppError};` (keeps `PaginationQuery` + pagination tests)
- `src-app/server/src/common/macros.rs` → moved macros re-exported at the crate root (see below); retains the ziee-coupled macros
- `src-app/server/src/core/app_state.rs` → `pub use ziee_core::app_state::{get_server_addr, set_server_addr};` + wrapper `get/set_app_data_dir` (T-1 app-name); retains CACHES_CONFIG + MAX_FILE_UPLOAD
- `src-app/server/src/lib.rs` + `src-app/server/src/main.rs` → `pub use ziee_core::{impl_json_from, impl_string_to_enum, sse_event_enum};` at each crate root so `crate::sse_event_enum!` etc. resolve unchanged
- `src-app/server/Cargo.toml` → adds `ziee-core = { path = "../../sdk/crates/ziee-core" }`

## Design-gate

**Mechanical.** No design gate to resolve (unlike B1b/B2/B3). The only genericity
introduced is **T-1**: the hardcoded `~/.ziee` data-dir default becomes a
configurable app-name (`set_app_name`) so the SDK is app-agnostic; ziee registers
`"ziee"` via a `Once` guard threaded through the data-dir accessors, preserving
`~/.ziee` byte-for-byte. `AppError`/`ApiError` are NOT part of the OpenAPI surface
(no `JsonSchema` derive), so the generated `types.ts` is unaffected — B1 is
equivalence-preserving with no schema/name movement (decision N2 satisfied).

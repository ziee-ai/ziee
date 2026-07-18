# Chunk B1 — TRANSFORMS (every non-byte-identical change + rationale)

The moved code is byte-identical to its pre-extraction ziee form EXCEPT the
transforms below. Everything else (`AppError` + all impls, `sse_event_enum!`
body sans the one path, `impl_string_to_enum!`, `impl_json_from!`, the app-state
globals + accessors, all moved tests) is a verbatim copy.

- **T-1** `APP_DATA_DIR`: the hardcoded `~/.ziee` default becomes `~/.<app_name>`, where `<app_name>` comes from a new `set_app_name(..)` / `APP_NAME: OnceLock<String>` (fallback `"app"`). — **why:** the SDK must be app-agnostic (a second consumer, CytoAnalyst, uses `~/.cytoanalyst`). ziee registers `"ziee"` before the first data-dir access via a `std::sync::Once` guard (`ensure_app_name`) threaded through the retained `get/set_app_data_dir` shims, so the default resolves to `~/.ziee` **byte-for-byte** — identical observable behavior. The moved `test_app_data_dir` sets the path explicitly, so it is app-name-independent.

- **T-2** `sse_event_enum`: the macro body's helper path `$crate::common::macros::pascal_to_camel_case` becomes `$crate::macros::pascal_to_camel_case`. — **why:** the macro is `#[macro_export]`, so `$crate` expands to the DEFINING crate (`ziee_core`) at every ziee call site; the helper now lives at `ziee_core::macros::pascal_to_camel_case`. The expansion is otherwise identical (same `OnceLock` cache, same camelCase conversion, same `Into<sse::Event>`); output is unchanged.

- **T-3** `doc-fences`: the two illustrative doc-comment code fences on `ApiResult` and `sse_event_enum` (which reference `crate::common::type` and fictional types `MyResponse`/`SomeData` that never compiled) are re-fenced from rust to ignore. — **why:** in ziee these examples were never run as doctests; in the standalone SDK `cargo test` would try to compile them and fail. Fencing `ignore` keeps the illustration without a spurious doctest failure. Doc-comment-only; zero effect on the API/type/behavior surface.

- **T-4** `ApiError`: deliberately NOT included in the ziee shim's `pub use ziee_core::{ApiResult, AppError};` re-export. — **why:** `ApiError` was only used internally by `AppError::into_response` (now in ziee-core); nothing in ziee references `common::type::ApiError` (its sole textual occurrence is a doc comment). Re-exporting it would trip the workspace's `unused_imports = "deny"` lint. `ApiError` remains public in `ziee_core::error`.

- **T-5** `ziee-core-dep`: `ziee-core = { path = ... }` added to `src-app/server/Cargo.toml`, plus `pub use ziee_core::{impl_json_from, impl_string_to_enum, sse_event_enum};` at BOTH `lib.rs` and `main.rs` crate roots. — **why:** the `ziee` package builds a lib AND a bin from the same module tree, so both crate roots must re-export the moved `#[macro_export]` macros for `crate::sse_event_enum!` to resolve; this wires the path dep + preserves the `crate::`-qualified macro call sites unchanged (the shim mechanism of decision N2).

## Decision

**Question:** How is `~/.ziee`-vs-app-agnostic (T-1) reconciled with byte-identical
equivalence, and where does the app-name get set so no read ever observes the
`"app"` fallback?

**Resolution:** The app-name is a runtime `OnceLock` set by the consumer. ziee's
retained `core/app_state.rs` shim wraps BOTH `get_app_data_dir` and
`set_app_data_dir` with a one-time `ensure_app_name()` (`Once::call_once` →
`ziee_core::app_state::set_app_name("ziee")`). Because every one of ziee's 33
data-dir call sites goes through these shims (never `ziee_core::app_state::*`
directly), the `"ziee"` app-name is guaranteed registered BEFORE the
`APP_DATA_DIR` `Lazy` first initializes — so the default is `~/.ziee`, identical
to the pre-extraction hardcode. The `"app"` fallback is unreachable from ziee.
This makes T-1 an equivalence-preserving genericization (DECISION N2 branch a).

Zero unresolved markers remain; every transform carries a rationale.

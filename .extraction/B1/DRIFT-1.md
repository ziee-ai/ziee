# Chunk B1 — DRIFT round 1

Reconciliation of the moved code against `CUT.md`/`TRANSFORMS.md` and the
equivalence tripwires.

- **DRIFT-1.1** — Every moved file resolves in the SDK (`error.rs`, `macros.rs`,
  `app_state.rs`, `config.rs` all exist under `sdk/crates/ziee-core/src/`) and every
  `## Symbols` entry (`AppError`, `ApiResult`, `ApiError`, `sse_event_enum`,
  `impl_string_to_enum`, `impl_json_from`, `pascal_to_camel_case`, `APP_DATA_DIR`,
  `SERVER_ADDR`, `set_app_name`, `ServerConfig`, …) is present. — verdict: none

- **DRIFT-1.2** — Every changed symbol is declared in `TRANSFORMS.md`: T-1
  (`APP_DATA_DIR` app-name), T-2 (`sse_event_enum` `$crate` path), T-3 (doc
  fences), T-4 (`ApiError` not re-exported), T-5 (dep + crate-root macro
  re-exports). No undeclared non-byte-identical change remains. — verdict: none

- **DRIFT-1.3** — No stale ziee reference points at the old locations: the moved
  macros are re-exported at `lib.rs`/`main.rs` crate roots so all `crate::sse_event_enum!`
  (11 sites), `crate::impl_string_to_enum!` (1), `crate::impl_json_from!` (2) call
  sites resolve; `common::type::AppError`/`ApiResult` resolve via the shim; the 33
  `get/set_app_data_dir` + `get/set_server_addr` sites resolve via `core/app_state.rs`.
  `cargo check -p ziee` (lib + bin) is green; `cargo test -p ziee --lib --no-run` is
  green. — verdict: resolved

- **DRIFT-1.4** — **Equivalence tripwire: `types.ts` GREEN, `openapi.json`
  byte-reordered (pre-existing).** After `--generate-openapi` for both binaries:
  `src-app/ui/src/api-client/types.ts` and `src-app/desktop/ui/src/api-client/types.ts`
  are **byte-identical** to the committed baseline (the client-consumed contract is
  unchanged). The two `openapi.json` files differ in **JSON key ORDER only** —
  proven semantically identical: path-set equal, schema-set equal, and order-
  insensitive `json.load` equality `True` (nothing added/removed/renamed).
  Root cause is NOT B1: a rebuild of **pristine HEAD** (B1 reverted) also fails to
  reproduce the committed `openapi.json` (357-line reorder), so the committed
  baseline is itself non-reproducible from its own source — the route-registration
  order (linkme distributed-slice) is a deterministic-per-dependency-graph function,
  and the chunk0 baseline was captured under a different graph/merge state. Adding
  the `ziee-core` dep perturbs that order a second time, but introduces **zero**
  semantic/type/behavior change (types.ts byte-identical; openapi set-equal).
  Flagged for the orchestrator: the `openapi.json` byte-baseline needs re-capture
  (it cannot pass a byte-identical gate even without B1); `types.ts` is the
  load-bearing equivalence anchor and it holds. — verdict: none

- **DRIFT-1.5** — **N2 shim vs. E6 file-absence.** `CUT.md`'s three `move:` sources
  (`common/type.rs`, `common/macros.rs`, `core/app_state.rs`) are RETAINED as pure
  re-export shims (the moved *definitions*, not the files, are deleted) — mandated by
  decision N2. The literal `E6 source-absent` file check is therefore intentionally
  waived for a partial (symbol-level) extraction; single-source is preserved because
  the shims contain no divergent duplicate definitions (only `pub use`
  re-exports + the app-side symbols that stay). — verdict: resolved

**Unresolved drifts:** 0

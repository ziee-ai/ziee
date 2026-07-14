# Chunk `health` — BOUNDARY

What `ziee-health` may and may not name, and why the split keeps the
app-agnostic + build-DB-free boundary clean.

## `ziee-health` is domain-free AND build-DB-free

- `types.rs` deps: `serde`, `schemars`. `HealthResponse` is a plain wire struct.
- `handlers.rs` deps: `aide::transform::TransformOperation`, `axum::{Json,
  debug_handler, StatusCode}`, `super::types::HealthResponse`, `serde_json` (test).
  `health_check` is a pure async fn — no DB, no auth extractor, no `Config`.
- `routes.rs` deps: `aide::axum`, `super::handlers`. A static route builder.
- Grep confirms: no `crate::modules`, no `ziee_core`/`ziee_identity`/
  `ziee_framework`, no `sqlx`, no `query!`, no build.rs.

## The boundary line — what stayed app-side

`health/mod.rs` keeps the `HealthModule` `AppModule` impl + the
`#[distributed_slice(MODULE_ENTRIES)]` registration (name "health", order 85).
This is the ONLY tie to ziee: `MODULE_ENTRIES` is ziee's `linkme` slice and
`ModuleContext`/`AppModule` live in `crate::module_api`. Every SDK-extracted
module keeps this glue app-side (the ziee-auth / ziee-control-mcp precedent) — the
crate is the reusable engine, the app owns the registration.

## E-gates (this chunk)

- **E (cargo):** `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk &&
  cargo check --workspace` = 0.
- **E8 (golden, BOTH surfaces):** `types.ui.ts` + `types.desktop.ts`
  **BYTE-IDENTICAL**; `openapi.ui.json` + `openapi.desktop.json`
  **CANONICALLY-EQUAL** (jq -S) vs `.extraction/baseline/`. The move touches no
  route/schema/permission-string/OpenAPI-visible type — nothing observable
  changed. Generated paths restored via `git checkout`.
- **test-fidelity:** `cargo test -p ziee-health` → 5 passed.

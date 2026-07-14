# Chunk `hardware` — BOUNDARY

What `ziee-hardware` may and may not name, and why the split keeps the
app-agnostic + build-DB-free boundary clean.

## `ziee-hardware` is domain-free AND build-DB-free

- `types.rs` deps: `serde`, `schemars`, `axum` (the SSE `Event` the
  `ziee_core::sse_event_enum!`-generated `Into` impl uses).
- `detection.rs` deps: `std::process`, `serde_json`, and the GPU libs
  (`nvml-wrapper`/`opencl3`/`ash`/`wgpu-hal`) behind the `gpu-detect` feature.
  Pure host probing — no DB, no app type.
- `monitoring.rs` deps: `axum`, `sysinfo`, `tokio`, `uuid`, `lazy_static`.
  Process-global SSE fan-out — no DB, no app type.
- `permissions.rs` deps: `ziee_identity::PermissionCheck`. Static const keys.
- Grep confirms: no `crate::modules`, no `sqlx`/`query!`, no build.rs.

The only SDK deps are `ziee-core` (the shared SSE macro) + `ziee-identity` (the
permission trait) — the dependency direction the plan prescribes (framework/
identity for permissions; core for shared macros).

## The boundary line — what stayed app-side

`hardware/{handlers,routes}.rs` name `RequirePermissions<(HardwareRead,)>` /
`with_permission::<…>` — ziee type-aliases that FIX the concrete
`ZieeIdentityResolver` (backed by the global `Repos` + `Arc<JwtService>`) — plus
`crate::common::{ApiResult, AppError}` and `sysinfo`. Moving them would require
making the axum handlers generic over the resolver — a rewrite, not an
equivalence-preserving move — so they STAY in ziee, along with the `HardwareModule`
`AppModule` registration (name "hardware", order 75). The crate is the reusable
detection/monitoring ENGINE; the app owns the permission-gated HTTP boundary.

## E-gates (this chunk)

- **E (cargo):** `cargo check -p ziee` = 0, `-p ziee-desktop` = 0, `cd sdk &&
  cargo check --workspace` = 0.
- **E8 (golden, BOTH surfaces):** `types.ui.ts` + `types.desktop.ts`
  **BYTE-IDENTICAL**; `openapi.ui.json` + `openapi.desktop.json`
  **CANONICALLY-EQUAL** (jq -S) vs `.extraction/baseline/`. The move touches no
  route/schema/permission-string/OpenAPI-visible type. Generated paths restored via
  `git checkout`.
- **feature-parity:** ziee's `default = [gpu-detect, …]` forwards to
  `ziee-hardware/gpu-detect`, so the stock app + desktop build compile the exact
  detection code paths.
- **test-fidelity:** `cargo test -p ziee-hardware --features gpu-detect` → 11
  passed / 1 ignored.

# DRIFT-1 — implementation vs plan

Reconciliation of divergences found after implementing all ITEMs and passing
`cargo check -p ziee --tests` (green) + `tsc` (ui, green).

- **DRIFT-1.1** — verdict: impl-wins — ITEM-7 OpenAPI mechanism. Plan said run
  `just openapi-regen`. A clean regen produced only a 1-line content delta (the
  description) plus ~157 lines of pre-existing positional key-order churn, and the
  operation description does not appear in `api-client/types.ts` (types.ts is
  byte-identical after regen for both ui + desktop). So the description was applied
  via a **surgical one-line edit** of both committed `openapi.json` files instead —
  minimal diff, no incidental reorder, no Tauri build, and the `types_ts_parity` /
  `types_ts_parity_desktop` golden tests stay green. PLAN_AUDIT "OpenAPI regen"
  section updated to record this. Resolved.

- **DRIFT-1.2** — verdict: resolved — adding `max_file_upload_mb` to
  `TestServerOptions` (ITEM-11) broke two EXHAUSTIVE struct initializers in the
  code_sandbox test fixtures (`tests/code_sandbox/{mirror_fixture,harness}.rs`,
  which don't use `..Default::default()`). Fixed by adding `max_file_upload_mb:
  None` to both. This is expected fallout of the field addition (part of ITEM-11),
  now compiling. Resolved.

- **DRIFT-1.3** — verdict: resolved — ITEM-3 wiring required re-exporting the new
  app_state functions from `core::mod` (`get_max_file_upload_bytes` /
  `set_max_file_upload_bytes` / `file_upload_body_limit_bytes`). `core/mod.rs` was
  edited accordingly (a supporting change under ITEM-2/3, not a new ITEM). Resolved.

- **DRIFT-1.4** — verdict: none — ITEM-7 also mentioned the `upload_file_docs`
  description; on inspection it cites no size number, so it was correctly left
  unchanged (already noted in PLAN_AUDIT). No divergence.

**Unresolved drifts:** 0

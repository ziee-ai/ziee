# DRIFT-1 — implementation vs plan

Authored live during phase 5, item by item.

- **DRIFT-1.1** — verdict: none — ITEM-1 (`SandboxAvailability::explain()`)
  implemented in config.rs exactly as planned, with the sibling per-variant unit
  test (TEST-1).
- **DRIFT-1.2** — verdict: resolved — The plan (ITEM-2/3) said "include
  `config::init_status().explain()`" at each site. Implementation introduces a
  shared `config::not_initialized_error()` helper that all 7 sites (SDK execute +
  6 ziee sites) route through, rather than duplicating the `format!` inline. This
  is a DRY refinement fully consistent with the plan's intent (one honest source
  of truth) and DEC-3; no plan amendment needed. `crate::common::AppError` ==
  `ziee_core::AppError` (verified) so the helper's return type fits every site,
  including the `(StatusCode, AppError)` tuple sites in version_handlers.
- **DRIFT-1.3** — verdict: none — ITEM-4 (seccomp classifier) implemented as the
  planned `Complete | ChildGone | Truncated` split; the genuine-truncation
  `error!` branch is preserved verbatim; TEST-3 covers both branches incl. the
  loud-stays-loud trap.
- **DRIFT-1.4** — verdict: none — ITEM-5 (`redact_host_paths`) implemented and
  applied to `result.stdout`/`result.stderr` in `execute_command_with_mounts`
  using `state.workspace_root` + `ensure.mount_dir`; TEST-4/5 cover it.
- **DRIFT-1.5** — verdict: resolved — Incidental: `use axum::http::StatusCode` in
  the SDK `execute.rs` became test-only after the guard was routed through the
  helper, so it is now `#[cfg(test)]`-gated to avoid an unused-import warning.
  Mechanical, no behavior change.

**Unresolved drifts:** 0

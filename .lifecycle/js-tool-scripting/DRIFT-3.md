# DRIFT-3 — js-tool-scripting (admin-configurable limits increment)

Implementation of ITEM-16..27 vs the amended plan. Minor, all reconciled.

- **DRIFT-3.1** — verdict: resolved — the API row type is generated as `JsToolSettings`,
  which collides with the store's registered name `JsToolSettings` (TS merged-declaration
  error). Resolved by importing the row type aliased as `JsToolSettingsRow` in the store +
  section; the store/registration name stays `JsToolSettings` (as planned in ITEM-26).
- **DRIFT-3.2** — verdict: impl-wins — PLAN/DEC pointed at code_sandbox's `SandboxSettingsPage`
  which is `data-page` in the gallery coverage (a seeded gallery cell across loaded/empty/error).
  The js-tool page is instead marked `kind: 'flow'` (covered by the e2e settings spec) — a
  lighter but honest coverage kind: the page's loaded/loading/no-permission states are exercised
  by TEST-50..53 against the real backend rather than a mock-cassette gallery cell. `npm run check`
  (gallery-coverage + state-matrix) passes with the `via` section + `flow` page + the two
  `JsToolSettingsSection:{delayed,error}` stateCoverage skips.
- **DRIFT-3.3** — verdict: resolved — TEST-53 (non-admin gate) planned to grant `js_tool::use`,
  but `JsToolUse` is not surfaced to the TS `Permissions` enum (the `/run-js/mcp` route uses a
  plain `route()` with no `with_permission` docs, so the perm never enters the spec). The e2e
  instead grants an unrelated `Permissions.SkillsRead` — the assertion (lacks
  `js_tool::settings::read` → forbidden) is unchanged.
- **DRIFT-3.4** — verdict: none — `run()` reads `settings_cache::get()` directly for
  `max_concurrent_runs` to prime the global admission sem, rather than carrying it on `JsCaps`.
  This matches DEC-22/23: the global cap is process-global (not a per-run cap), so it is not a
  `JsCaps` field; the two per-run caps (`max_concurrent_dispatch`, `max_trace_entries`) ARE on
  `JsCaps` as planned.

**Unresolved drifts:** 0

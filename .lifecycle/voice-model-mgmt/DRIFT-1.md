# DRIFT-1 — implementation vs plan (voice-model-mgmt)

Reconciliation of the implemented backend + frontend against PLAN.md. Each
divergence is classified `plan-wins` (re-implement), `impl-wins` (amend plan), or
`resolved`/`none`.

## Backend
- **DRIFT-1.1** — verdict: impl-wins — Model-name validation is SPLIT: `validate_settings_patch`
  does the pure storable-FORMAT check (`is_valid_model_name`, 1..=50, `[A-Za-z0-9._-]`); the
  catalog-or-installed EXISTENCE check is async in the `update_settings` handler (a pure validator
  can't hit the DB). PLAN ITEM-9 implied one validator; the split is the only correct shape. TEST-5
  is a unit test of the format helper; the existence-rejection is exercised by the settings
  integration path — no plan change beyond this note.
- **DRIFT-1.2** — verdict: resolved — ITEM-31/F9 delivered the MODEL download poll-snapshot
  (`GET /voice/models/downloads/{key}`, `get_model_download`) AND backfilled the binary-version
  snapshot (`GET /voice/versions/downloads/{key}`). Both present.
- **DRIFT-1.3** — verdict: resolved — ITEM-32/F10 `detect-gpu` NARROWED OUT (DEC-19, recorded +
  test adjusted): redundant with `check-updates`, and a real available-backends list needs an
  upstream release-asset fetch. `GET /voice/versions/{id}` + instance `pid`/`uptime_seconds` ARE
  delivered.
- **DRIFT-1.4** — verdict: impl-wins — The unified `download_model_file` fixed the legacy
  temp-leak (cleanup off the Err-only branch) as part of ITEM-25; the legacy `ensure_model` path
  (catalog auto-download for the configured model) is retained unchanged for the auto-start flow.
  No plan change.

## Frontend (delegated; deviations reported + accepted)
- **DRIFT-1.5** — verdict: impl-wins — Catalog list is owned by `VoiceModelUpdate.store` (single
  source of truth for `AvailableModelsCard`), mirroring how `VoiceUpdate` owns the engine-version
  feed. PLAN ITEM-12 listed `VoiceModel.loadCatalog`; it exists but delegates to
  `VoiceModelUpdate.checkForUpdates()` rather than duplicating catalog state. Cleaner; accepted.
- **DRIFT-1.6** — verdict: impl-wins — Catalog initial load + `sync` subscription live in
  `VoiceModelUpdate.init(ctx)`, NOT a card `useEffect` (the sibling `AvailableVersionsCard` uses
  `useEffect`, but the house rule forbids useEffect data-loading). Correct per REACT_COMPONENT_PATTERNS.
- **DRIFT-1.7** — verdict: impl-wins — Instance logs UI uses `getInstanceLogs` (fetch + refresh
  button), NOT the SSE `streamInstanceLogs` (PLAN ITEM-33 marked streaming optional). The backend
  `streamInstanceLogs` endpoint exists for API completeness but is currently unused by the UI.
- **DRIFT-1.8** — verdict: resolved — `ListPagination` import path corrected to the real shared
  location `@/components/common/ListPagination` (PLAN pointed at an llm-provider-relative path).

## Verification performed (P1 — re-ran the artifacts myself)
- Backend `cargo check -p ziee`: clean (0 errors, 0 voice warnings) — logs voice-check-4/5.
- Server + desktop OpenAPI regen: new `/voice/models/*` + version routes present; old sync
  `/voice/model/download` removed; both `types.ts` regenerated (12 new `Voice.*` methods).
- Frontend `npx tsc --noEmit`: clean (exit 0), independently re-run.

## Not-yet-done (Phase 8 scope, not drift)
The enumerated integration + e2e tests (TEST-6..13, 17..37 minus the inline unit tests already
written: TEST-1 catalog parse, TEST-4 download-task) are Phase-8 deliverables, not Phase-5
implementation. Inline unit tests still to add in Phase 8: TEST-2 (magic/cap), TEST-5 (name/repo
format). Recorded here so they are not mistaken for silent omissions.

**Unresolved drifts:** 0

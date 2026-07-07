# DRIFT-1 — implementation vs plan

Audited the implemented diff against PLAN.md / DECISIONS.md / TESTS.md after
completing all items. Compilation is green across all touched surfaces (server
lib `cargo check`, integration-test `cargo check`, `ui` + `desktop/ui` `tsc`,
13 node unit tests, both OpenAPI regens).

- **DRIFT-1.1** — verdict: resolved — ITEM-7 draft SAVE cadence. DEC-7 originally
  said "save on input debounced ~300ms". The implementation saves immediately on
  each input event (no debounce). Reconciled by amending DEC-7: localStorage
  writes are cheap + synchronous and immediate save is more deterministic for the
  e2e. DECISIONS.md updated; phase-4 gate re-run.
- **DRIFT-1.2** — verdict: resolved — TEST-11 scope. TESTS.md originally mapped
  the content-search e2e to `ITEM-6, ITEM-4`. Seeding real message CONTENT in an
  LLM-free e2e is not reliably possible (the POST-message path invokes the
  provider). Rescoped TEST-11 to prove the SERVER-SIDE search wiring (ITEM-6) via
  a pagination trick (a uniquely-titled conversation on a later page is found by
  search — impossible with the old client-only filter). ITEM-4's content-vs-title
  matching stays covered against a real DB by integration TEST-1/TEST-2. TESTS.md
  updated; phase-3 gate re-run.
- **DRIFT-1.3** — verdict: none — ITEM-3 scope. The MCP tool-result card was
  confirmed to ALREADY collapse (`isExpanded` + `max-h-40`), so ITEM-3 was scoped
  to long text bubbles only (PLAN amended in phase 1 before coding). Implemented
  exactly that; no divergence.
- **DRIFT-1.4** — verdict: impl-wins — desktop `openapi.json` regen also
  corrected a PRE-EXISTING stale `GET /projects` `search` param (the committed
  desktop spec had drifted from the desktop binary before this branch). This is a
  deterministic, source-faithful regen output; keeping it is correct (the golden
  parity test requires the generated file to match source). Not a product change
  of this feature — a benign side-effect of running the canonical regen. No plan
  amendment needed; recorded for transparency.
- **DRIFT-1.5** — verdict: none — added `src/modules/chat/components/collapsible.ts`
  (pure threshold helper) and `findMatches.ts` (pure match helper) beyond the
  PLAN's file list, to make ITEM-1/ITEM-3 logic unit-testable (TEST-6, TEST-7).
  Consistent with the plan's intent (the tests were enumerated against these
  helpers); the extra files are the test seams, not scope creep.
- **DRIFT-1.6** — verdict: none — ITEM-8 composed `FileUploadArea` +
  `FilePasteHandler` into one `ComposerFileListeners` slot entry because a slot
  takes one component per extension. Anticipated in PLAN_AUDIT; both listeners
  attach independently to `[data-chat-composer]`. No behavior change to drag-drop.

**Unresolved drifts:** 0

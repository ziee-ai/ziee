# DRIFT-1 — office-run-office-js (implementation vs plan)

Audit of the implemented diff against PLAN.md after the first implementation pass.

- **DRIFT-1.1** — verdict: impl-wins — The pane reply and `serializeResult` return
  `{ result, truncated, text }`, not the literal `{ result, truncated }` planned:
  the `text` (capped string) lets `pane_tool_result` surface the value in the
  readable `content` channel too (not only `structuredContent`), consistent with
  DEC-7's "wrapped via `pane_tool_result`". PLAN ITEM-6/ITEM-7 amended to
  `{ result, truncated, text }`; re-ran phase 1/3 gates.

- **DRIFT-1.2** — verdict: impl-wins — Added a named `describeError(prefix, e)`
  pane helper (Office.js `name`/`message`/`.code`/`.debugInfo` → one structured
  string) rather than inlining the structured-error assembly. This is exactly
  ITEM-6's "STRUCTURED message assembled from the Office.js error" (DEC-9), just
  factored into a node-testable pure helper (also exported + covered by TEST-9).
  PLAN ITEM-6/7 amended to name it; no behavior change.

- **DRIFT-1.3** — verdict: none — Removing `edit_document` (ITEM-4) orphaned the
  `parse_args` helper in `handlers.rs` (its only caller was `EditDocumentArgs`
  parsing); removed it. Within ITEM-4's scope ("remove the `edit_document`
  dispatch arm … drop the now-unused imports"); the dead helper is the same kind
  of fallout. No plan change needed.

- **DRIFT-1.4** — verdict: none — Removing `act_on_document` (ITEM-5) orphaned the
  Windows `com_call` helper and the macOS `applescript_escape` helper (+ its unit
  test); removed all three. ITEM-5 already enumerated "supporting helpers (macos
  append-osascript helper, windows COM append helper)"; `com_call`/`applescript_escape`
  are precisely those. Verified the remaining warnings (`not_implemented_err`,
  `not_supported_err`) are PRE-EXISTING (their users live in `unsupported.rs`,
  `#[cfg(not(any(windows, macos)))]`, so unused on this mac build regardless of
  this diff) — NOT introduced here. No plan change needed.

- **DRIFT-1.5** — verdict: none — `run_office_js` was implemented as its OWN
  dispatch arm (separate from the `"read_document" | "get_selection"` arm) because
  it adds the non-empty-`script` `INVALID_ARGS` validation before the round-trip.
  This matches ITEM-3's intent (host-agnostic pane route + arg validation mirroring
  the removed `edit_document` check); the "add to the host-agnostic pane arm"
  phrasing was descriptive, not prescriptive about arm-sharing.

**Unresolved drifts:** 0

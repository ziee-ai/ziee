# PLAN_AUDIT — office-run-office-js

Audit of PLAN.md against the codebase (grep-verified) before writing any code.

## Breakage risk

- **`edit_document` references (removal fallout).** Repo-wide grep found all
  references confined to the office_bridge module + its tests:
  - `tools.rs` (descriptor + header + test) — removed by ITEM-1/2/8.
  - `handlers.rs` (dispatch arm + 2 unit tests + capability doc block) — removed
    by ITEM-4/8.
  - `tests/office_bridge/settings_mcp_test.rs::EXPECTED_TOOLS` + its `tools/list`
    HTTP assertion — updated by ITEM-2 (caught in this audit; plan amended).
  - `bridge/protocol.rs:6` — a stale example doc comment only — updated by ITEM-8.
  No production caller outside the module; no external crate imports the tool by
  name. Removal is self-contained.
- **`act_on_document` / `DocOp` / `ActResult` (dead-path removal).** grep confirms
  the SOLE production consumer is `handlers.rs:257` (the `edit_document` arm).
  Remaining references are the trait def + 4 impls + 2 direct `#[cfg(test)]` tests
  (`platform/mod.rs`, `platform/unsupported.rs`) + the macos/windows append
  helpers — all removed together by ITEM-5. `ActResult` has no consumer beyond
  `act_on_document`, so it dies with it. No dangling refs after removal.
- **`run_office_js` name collision.** grep: zero pre-existing uses anywhere — the
  new tool name / pane method string is free.
- **Broker/protocol change?** None. `broker::call_pane(doc, method, params)` is
  generic over the method string and `taskpane.js`/`server.rs` classify frames by
  shape, not method — `run_office_js` is just a new method value. No change to
  `broker.rs`, `server.rs`, or `protocol.rs` logic (protocol.rs is doc-only).

## Pattern conformance

- ITEM-1/2 (descriptor): mirrors the `read_document`/`get_selection` descriptors
  in the SAME `tools.rs`; the `tool_list` unit test mirrors
  `tool_list_contains_all_seven_tools`. Conforms.
- ITEM-3 (dispatch): the host-agnostic pane arm (`"read_document" |
  "get_selection"` → `require_doc_full_name` → `broker::call_pane` →
  `pane_tool_result`) is the exact seam `run_office_js` slots into; arg-validation
  mirrors the removed `edit_document` non-empty check. Conforms.
- ITEM-6/7 (pane handler): mirrors `opReadDocument` (host `Word.run`/`Excel.run`
  branches, `reply`/`replyErr`, `.catch` structured error, `capText`/`MAX_READ_CHARS`
  cap). `serializeResult` joins the existing `module.exports` pure-helper block.
  Conforms.
- ITEM-5 (removal): reverse of how ITEM-9 added the pane path; no new pattern.
- Tests: mirror `pane_rpc_test.rs` mock-pane tests + the `#[cfg(target_os="macos")]
  #[ignore]` live test, and `taskpane.test.mjs` pure-helper node tests. Conform.

## Migration collisions

None. This feature touches no SQL migration, no DB table, no permission grant.
`ls migrations/` is irrelevant here — the office_bridge tool surface is runtime
JSON (`tool_list()`), not schema. No collision.

## OpenAPI regen

- The MCP tool surface (`tool_list()` / `tools/call`) is **runtime JSON-RPC, not
  an OpenAPI schema** — adding/removing tools does NOT touch any `#[derive(JsonSchema)]`
  type, so on its own it needs no regen.
- **BUT** ITEM-8 rewords the `OpenDoc.full_name` doc comment (it currently
  references the removed `act_on_document`). `OpenDoc` IS a REST OpenAPI schema —
  `GET /api/office-bridge/documents` returns `Json<Vec<OpenDoc>>`, and schemars
  carries the field doc comment into `desktop/ui/openapi.json` +
  `types.ts` JSDoc. So this feature **requires `just openapi-regen` for the
  DESKTOP spec only** (office_bridge types live in the desktop crate; the `ui`
  spec has no `OpenDoc`, confirmed by grep — its regen output is unchanged).
  Handled: ITEM-8 lists the reword + the mechanical regen output. Run in phase 8;
  golden `types_ts_parity` test must stay green.

## Deferred / decision-needing items (→ Phase 4 DECISIONS)

- Shared `broker::CALL_TIMEOUT = 15s` may be tight for a heavy `run_office_js`
  script (loop over hundreds of cells). Decide: keep the shared 15s (a timeout
  surfaces as the typed `OFFICE_PANE_TIMEOUT` the model self-corrects on by
  reducing scope) vs. a longer/​separate timeout. → DEC-1.
- Per-call approval for `run_office_js` — verify it needs no code (office_bridge
  absent from `is_builtin_server_id`). → DEC-2.
- `edit_document` removal drops the native no-pane append capability. → DEC-3.
- Read/write declared-intent split — defer to a follow-up? → DEC-4.
- `PowerPoint.run` availability / host-selection when `HOST === 'unknown'`. → DEC-5.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new descriptor mirrors existing pane-tool descriptors; no collision.
- **ITEM-2** — verdict: CONCERN — must ALSO update `settings_mcp_test.rs::EXPECTED_TOOLS` (found in audit); plan amended to include it. No blocker.
- **ITEM-3** — verdict: PASS — slots into the existing host-agnostic pane dispatch seam; approval inherited (DEC-2).
- **ITEM-4** — verdict: PASS — self-contained removal of the arm + struct + 2 tests; drop the now-unused `DocOp` import.
- **ITEM-5** — verdict: CONCERN — wider than one file (trait + 4 impls + 2 tests + `ActResult`); grep confirms no consumer survives, so removal is clean, but it must be done atomically or the crate won't compile. Sequence with ITEM-4.
- **ITEM-6** — verdict: PASS — mirrors `opReadDocument`; `new Function` async-body execution inside `{Word,Excel,PowerPoint}.run` is the standard Office.js embedding; structured error from the Office.js error object.
- **ITEM-7** — verdict: PASS — reuses `capText`/`MAX_READ_CHARS`; pure helper is node-testable via the existing `module.exports` seam.
- **ITEM-8** — verdict: CONCERN — the `OpenDoc` doc-comment reword triggers a DESKTOP `just openapi-regen` (per OpenAPI-regen dimension). Not a blocker; sequenced into phase 8. Pure doc/comment edits elsewhere.
- **ITEM-9** — verdict: PASS — doc-only checklist addition; same `taskpane.js` runs under WebView2, so the Windows step is a live-verify note, mirroring the existing `WINDOWS_PANE_VERIFICATION.md` structure.

No `BLOCKED` verdicts. Plan amended for the two CONCERNs that added scope
(ITEM-2 test const, ITEM-8 regen). Proceed to Phase 3.

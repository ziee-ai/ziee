# PLAN_AUDIT — office-mode-gated-approval

Audit of the plan against the codebase (grep-verified) before writing code.

## Breakage risk

- **Pruned-tool consumers are confined to the module + its tests.** `read_document` /
  `get_selection` / `add_comment` / `set_track_changes` / `get_tracked_changes` appear
  only in: `tools.rs` (descriptors), `handlers.rs` (2 dispatch arms + PPT pre-gate +
  unit tests), `taskpane.js` (op handlers), `settings_mcp_test.rs::EXPECTED_TOOLS`,
  and `pane_rpc_test.rs` (as round-trip vehicles). No external crate imports them by
  name. The chat/model reaches them only via `tool_list()`, which we shrink.
- **`pane_rpc_test.rs` nuance (audit-caught).** Several tests vehicle on the pruned
  tools: `test9`/`test12`/`test16` go through `dispatch_tool` → after pruning they'd
  hit `UNKNOWN_TOOL`, so they MUST retarget to `run_office_js` (ITEM-7). `test6`/`test7`/
  `test8`/`test15` call `broker::call_pane` directly with a method string the broker
  forwards to the mock pane verbatim, so they'd still pass with a dead name — retargeted
  anyway for cleanliness. `test10_pane_mediated_method_no_pane` loops over three pruned
  tools and is fully subsumed by `test6_run_office_js_no_pane` → removed (ITEM-3).
- **`broker::OFFICE_UNSUPPORTED_ON_HOST` must NOT be removed** — `run_office_js` +
  `read_document` map the pane's `-32002` through it; only the handlers.rs-local const
  (sole users: `unsupported_on_ppt_err` + the two removed PPT tests) is orphaned.
  `OfficeApp` import stays (used by `seeded_mock`). (Both re-confirmed by grep.)
- **Server approval-loop extraction (ITEM-5) is the highest-risk item** — it refactors
  code on the hot path of EVERY MCP tool call (`mcp.rs:2120`). It must be strictly
  behavior-preserving for control / builtin / disabled / normal-manual / auto-approved
  servers. Mitigation: extract to a pure function with exhaustive unit tests covering
  every pre-existing branch BEFORE adding the office branch, and diff-review the loop
  call site. → CONCERN, not a blocker.
- **`office_bridge_mcp_server_id()` duplication.** The server recomputes the same
  deterministic `Uuid::new_v5(NAMESPACE_URL, "office_bridge.ziee.internal")` the desktop
  `mod.rs` defines (the server can't depend on the desktop crate). Drift risk if either
  string changes. Mitigation: a desktop-crate test asserts the desktop row id equals
  `ziee::…::office_bridge_mcp_server_id()` (the desktop crate sees both). → CONCERN.
- **Spoof safety.** Gating the read-bypass on the office_bridge server-id (not tool-name
  alone) means a user-added MCP server that names a tool `run_office_js` does NOT get
  read-auto-approval. Verified by an explicit unit test (ITEM-7).

## Pattern conformance

- ITEM-5/6 mirror `control_mcp/handlers.rs` exactly: a pure `needs_approval_decision`
  classifier with in-source `#[cfg(test)]` (`reads_never_need_approval`,
  `mutating_*_always_needs_approval`). Conforms.
- ITEM-7 approval-loop integration mirrors `tests/mcp/mcp_approval_workflow_test.rs`
  (`create_test_mcp_server` / `set_mcp_settings` / `send_message_with_mcp` /
  `get_pending_approvals`). Conforms.
- ITEM-1/3/4 mirror the `edit_document` prune from `office-run-office-js` (same seams).
  Conforms.

## Migration collisions

None. No SQL migration. The `auto_approved_tools` per-conversation mechanism +
`ApprovalMode` already exist (migration-complete); this feature adds no table/column.
`ls migrations/` is irrelevant.

## OpenAPI regen

Not required. The tool surface (`tool_list()`) is runtime JSON-RPC, not an OpenAPI
schema; the `mode` field is a `tool_list` property, not a `#[derive(JsonSchema)]` type.
The server approval decision is internal logic. No `#[derive(JsonSchema)]`/`ToSchema`
type changes → no `openapi.json`/`types.ts` regen in either workspace. (Distinct from
`office-run-office-js`, which touched the `OpenDoc` doc-comment; nothing here touches
`OpenDoc` or any schema type.)

## Decision-needing items (→ Phase 4)

- `mode` required vs optional-default-write; exact bypass predicate (`== "read"`). → DEC.
- Where the server classifier + `office_bridge_mcp_server_id` live (new
  `office_approval.rs`). → DEC.
- Whether the daemon `dispatch_tool` should validate `mode` at all (plan says no —
  execution ignores it). → DEC.
- The accepted trade-offs (no enforcement / read-exfiltration / always-allow blast
  radius). → DEC.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — descriptor prune mirrors the edit_document removal; unit-test set shrinks to 2.
- **ITEM-2** — verdict: PASS — `mode` is an additive schema field + description copy; no execution change.
- **ITEM-3** — verdict: CONCERN — wider test surgery than one arm (2 arms + PPT pre-gate + const + several unit tests + `test10` removal); grep-confirmed sole users, done atomically so the crate compiles.
- **ITEM-4** — verdict: PASS — pane op removal mirrors the prior feature; shared helpers (`capText`/`ERR_UNSUPPORTED_HOST`) confirmed still used by `run_office_js`.
- **ITEM-5** — verdict: CONCERN — refactors shared hot-path approval code; behavior-preserving extraction gated by exhaustive unit tests + call-site review. No blocker.
- **ITEM-6** — verdict: CONCERN — security-critical classifier; the fail-safe (only exact `"read"` on the office_bridge server bypasses) + spoof test + missing/invalid-mode test lock it.
- **ITEM-7** — verdict: CONCERN — cross-crate test updates (desktop retargets + new server approval-loop integration + id-drift test); enumerated fully in TESTS.md.
- **ITEM-8** — verdict: PASS — doc/comment updates only.

No `BLOCKED` verdicts. The CONCERNs are scrutiny flags (shared approval code, cross-crate
tests, id duplication), each with an explicit mitigation. Proceed to Phase 3.

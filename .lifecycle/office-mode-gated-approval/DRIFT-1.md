# DRIFT-1 — office-mode-gated-approval (implementation vs plan)

- **DRIFT-1.1** — verdict: impl-wins — The plan's deterministic server-side office
  approval-loop integration tests (originally TEST-13..16: register office_bridge under
  its id in the server harness, drive read/write/always-allow/deny) are INFEASIBLE as
  specified: `office_bridge` is a **desktop-only** MCP server (not registrable in
  `server/tests/mcp`), and the approval-workflow harness is **LLM-driven** (a real model
  emits the tool call). PLAN_AUDIT under-called this. Amended (TESTS.md + PLAN.md
  Files-to-touch, re-gated phases 1–3): the office decision is instead pinned
  **exhaustively by the unit matrix** (TEST-10 `run_office_js_read_bypass` — every mode
  value, spoof server, fail-safe; TEST-12 `compute_needs_approval` — all 9 branches), the
  loop **wiring** is proven behaviour-preserving by the EXISTING
  `mcp_approval_workflow_test.rs` regression (TEST-13 — the same loop routes through the
  extracted fn), and a real-LLM test (TEST-14) validates the trust-model's core
  assumption (the model self-classifies `mode`).

- **DRIFT-1.2** — verdict: none — `ApprovalMode` gained `#[derive(Copy)]` (additive; a
  trivially-Copy fieldless enum) so `compute_needs_approval` can take it by value in the
  loop. Within ITEM-5's extraction; no behaviour change.

- **DRIFT-1.3** — verdict: none — Added a `ziee::chat_extension::office_bridge_mcp_server_id`
  re-export in `server/src/lib.rs` (the `mod modules` is private; the desktop crate reaches
  server items only via the curated `chat_extension` facade) so the ITEM-6 drift test
  (TEST-11) can compare ids across the crate boundary. Within ITEM-6.

- **DRIFT-1.4** — verdict: none — `OfficeApp` moved from the `handlers.rs` module import to
  the test-module import: after removing `doc_host` (ITEM-3) it is used ONLY by the
  `seeded_mock` test helper, so a module-level import would warn unused in non-test builds.
  Within ITEM-3's removal.

- **DRIFT-1.5** — verdict: none — Removed `test13_live_mac_pane_ops` from `pane_rpc_test.rs`
  (it drove the now-removed pane ops `get_selection`/`read_document`; its live-round-trip
  purpose is already covered by `run_office_js_live_mac_executes_script`). Within ITEM-7's
  test updates; recorded so it isn't mistaken for lost coverage.

**Unresolved drifts:** 0

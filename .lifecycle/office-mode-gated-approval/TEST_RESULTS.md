# TEST_RESULTS — office-mode-gated-approval

Backend-only diff (desktop `office_bridge` Rust + resource JS, server `mcp`; no
`src-app/ui`/`desktop/ui` source) → the backend chain applies; no `npm run check` line.

## Unit (deterministic)

- **TEST-1**: PASS — `tools.rs::tool_list_contains_exactly_the_two_tools` (2 tools, 5 pruned absent).
- **TEST-2**: PASS — `tools.rs::run_office_js_schema_has_mode_and_description_guidance` (mode enum+required + read/write/approval guidance in the description).
- **TEST-3**: PASS — `handlers.rs::test3_pruned_tools_are_unknown` (all 5 pruned names → UNKNOWN_TOOL).
- **TEST-4**: PASS — `handlers.rs::test4_run_office_js_mode_does_not_gate_execution` (read≡write at the daemon).
- **TEST-5**: PASS — `handlers.rs::test12_list_open_documents_returns_seeded_docs` (native tool intact).
- **TEST-6**: PASS — node `taskpane.test.mjs` (shared pane helpers survive the op removal).
- **TEST-10**: PASS — `office_approval.rs` `run_office_js_read_bypass` matrix (every mode value, spoof server, wrong tool, None id).
- **TEST-11**: PASS — `mod.rs::office_bridge_id_matches_server_recomputation` (desktop row id == server `office_bridge_mcp_server_id()`).
- **TEST-12**: PASS — `office_approval.rs` `compute_needs_approval` — ALL 9 branches (control read/write, builtin, office read/write/always-allow, Disabled fail-safe, ManualApprove ±auto, AutoApprove) + `office_bridge_is_not_approval_bypassed` invariant.

Runs: `cargo test -p ziee-desktop --lib office_bridge::` → **53 passed, 0 failed**;
`cargo test -p ziee --lib office_approval::` → **10 passed, 0 failed**; node → passed.

## Integration (desktop; Postgres build DB)

- **TEST-7**: PASS — `settings_mcp_test.rs::test7_tools_list_returns_the_two_office_tools` (live tools/list = 2).
- **TEST-8**: PASS — `pane_rpc_test.rs::run_office_js_dispatch_round_trip` (read & write both round-trip, `mode` passed through unchanged).
- **TEST-9**: PASS — `pane_rpc_test.rs::test9_dispatch_tool_run_office_js_round_trip` (+ the `-32002`→OFFICE_UNSUPPORTED_ON_HOST retarget).

Run: `cargo test -p ziee-desktop --test integration_tests -- --test-threads=1
office_bridge::settings_mcp_test::test7 office_bridge::pane_rpc_test` → **10 passed, 0 failed, 2 ignored** (the prior-feature live-mac tests).

## Real-LLM (coder.ziee `qwen3.6-35b-a3b` via SSH tunnel)

- **TEST-14**: PASS — `pane_rpc_test.rs::run_office_js_real_llm_declares_mode`. Ran against the
  live endpoint: the model declared `mode:"read"` for the read task and `mode:"write"` for the
  write task. **This test earned its keep**: on the first run the model mislabeled the WRITE task
  as `"read"` (which would have auto-run a change unapproved) — the tool `description` was then
  rewritten with an explicit "any assignment / mutating call → write; when in doubt → write" rule,
  after which qwen classifies both correctly. Soft-skips when `ZIEE_OFFICE_REAL_LLM_URL` is unset.

  Run: `ZIEE_OFFICE_REAL_LLM_URL=http://127.0.0.1:4000/v1/chat/completions cargo test -p ziee-desktop
  --test integration_tests -- office_bridge::pane_rpc_test::run_office_js_real_llm_declares_mode`.

## Not run — environment-gated (documented, compensated)

The EXISTING `server/tests/mcp/mcp_approval_workflow_test.rs` suite (which would exercise the
extracted `compute_needs_approval` through the live loop for control/normal servers) requires an
LLM provider API key — all 32 tests panic at test-model creation ("No AI provider API keys found",
`helpers.rs:593`) BEFORE reaching any assertion, so it cannot run in this environment. It is NOT a
regression (the failure is at provider setup, never touching this diff), and the extraction's
behaviour-preservation is proven DETERMINISTICALLY by **TEST-12** (all 9 branches reproduce the
original inline logic) plus the **phase-6 blind equivalence audit** (two independent reviewers,
hunk-by-hunk, both clean). office_bridge is desktop-only, so that server suite never flows an
office id through the loop anyway — the office branch is covered by TEST-10/TEST-12 + TEST-14.

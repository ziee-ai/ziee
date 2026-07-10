# DRIFT-1 — implementation vs plan

Divergences found while implementing + validating against the live container logs.
All resolved by amending PLAN.md/TESTS.md (impl-wins) — the implementation revealed
facts the plan got slightly wrong.

- **DRIFT-1.1** — verdict: impl-wins — ITEM-1 originally listed THREE non-executing
  branches (`server_id==None`, server-not-found, connect-fail). Implementation found a
  FOURTH in the same function — the sampling-no-session error branch — which also pushed an
  error result but never deleted the approval row (identical latent re-loop). Extended the
  fix + amended ITEM-1 to cover all four, and to also push `tool_use_id` into the returned
  `executed_tool_use_ids` in each (mirrors the success path).

- **DRIFT-1.2** — verdict: impl-wins — ITEM-3's recovery was planned only for the "no `__`
  separator" arm. The live logs show gpt-oss/harmony ALSO emits an empty-prefix form
  (`__query_rag`), which the split routes to the valid-prefix arm with `server_id=""` and
  would have bypassed recovery. Reworked `get_accumulated_content` to recover whenever the
  parsed prefix is **not a valid UUID** (covers both `execute_command` and `__query_rag`).
  Amended ITEM-3.

- **DRIFT-1.3** — verdict: impl-wins — the plan (and approved plan file) framed defect #2 as
  a "constant `tool_use` tool-call id". That was a MISREAD: the finalize log's `id={}` field
  was `tool_use.to_message_content().content_type()`, which returns the serde type tag
  `"tool_use"`, NOT the id. The `accumulate_delta` logs show the real gpt-oss ids are UNIQUE
  (`chatcmpl-tool-<hash>`). So Fix B (message-unique ids) is a **defensive** net for a model
  that emits empty/duplicate ids — not the confirmed root cause (which is entirely ITEM-3's
  prefix stripping). Kept Fix B (cheap, preserves good ids) but reframed ITEM-2 + the Context
  accordingly, and additionally fixed the misleading finalize log to print the real id.

- **DRIFT-1.4** — verdict: impl-wins — TESTS.md named `tests/mcp/approval_test.rs` /
  `mcp_streaming_workflow_test.rs` / `mcp_approval_workflow_test.rs` for the integration
  tests. `approval_test.rs` is NOT registered in `tests/mcp/mod.rs` (marked "outdated
  TestServer API"). Consolidated the integration tests into a NEW registered file
  `tests/mcp/mcp_approval_loop_test.rs` with two end-to-end tests (bare-name-recovers-executes
  + unresolvable-errors-terminates) driven by the `oai_capture_stub` + `mock_mcp_server`
  harness; dropped the separate TEST-7/TEST-9 (folded into the two, and the pure logic is
  already covered by unit TEST-1..6). Amended TESTS.md + PLAN.md Files-to-touch.

**Unresolved drifts:** 0

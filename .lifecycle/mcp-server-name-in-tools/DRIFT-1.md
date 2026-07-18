# DRIFT-1 — implementation vs plan

- **DRIFT-1.1** — verdict: impl-wins — DEC-10 planned to extend `RecordedRequest`
  (`tests/common/stub_chat.rs`) with a `tool_descriptions` field to observe the
  per-tool label. During implementation I found `common::oai_capture_stub::StubChat`
  already captures the **verbatim** `/v1/chat/completions` body (tools WITH
  descriptions + system messages) and scripts tool calls — exactly what
  `mcp_approval_loop_test.rs` uses. So the integration tests use that stub and
  `common::stub_chat` is left UNTOUCHED (strictly smaller diff, no shared-harness
  edit). DECISIONS.md DEC-10 amended to record the revised approach.

- **DRIFT-1.2** — verdict: resolved — TEST-4 initially read `stub.last_request()`,
  but a background conversation-title-generation LLM call also hits the stub
  (WITHOUT MCP tools) and became "last", so the external tool wasn't found. Fixed by
  selecting the request that actually carries the `__search_bio` tool via
  `stub.requests()` — robust against extra background calls. The server log
  independently confirmed the product behavior BEFORE the test fix:
  `Tool 0: name='<uuid>__search_bio', description='[biognosia] Search the biology
  corpus'`. Pure test-observation refinement; no product-code change.

- **DRIFT-1.3** — verdict: none — TEST-3 (`connected_servers_section`) placed in the
  existing outer `mod tests` (next to its `tool_system_guidance` sibling) rather than
  a new module; a mechanical placement detail, not a plan divergence.

**Unresolved drifts:** 0

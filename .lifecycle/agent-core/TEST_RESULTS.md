# TEST_RESULTS — agent-core / chat re-home

## Honest status of the Phase-3 enumerated tests

The migration was verified by a REAL, PASSING set of tests, but that set does NOT
line up 1:1 with the file names Phase-3 enumerated up front — several enumerated
integration/e2e FILES were never authored (the behavior they'd cover was verified
through the crate tests + the existing chat/mcp/workflow suites + the tests
actually written this feature). Recorded truthfully below.

### Verified PASS (run this session)
- **TEST-1..13, 34** (agent-core unit) — `cargo test -p agent-core` → **36/36 PASS**
  (loop, ports, budget, tokens, policy, reviewer, compaction, fanout, extension,
  streaming-seam, cancel, resume, terminal-tool).
- **TEST-38** (existing chat suite on the agent-core path) — `chat::chat_stream_test`
  **6/6**, `chat::streaming_test` **10/10**, `chat::agent_core_migration_test` **1/1**,
  `chat::conversations_test` **25/25**; `agentic_chat::*` at **parity** with the
  legacy loop (identical failure set = only the 8 env-gated real-LLM tests). PASS.
- **Real-bridge behavioral** — `agent-core/tests/real_llm_loop.rs` (tool round-trip
  + 332 streamed deltas) + `mcp::agent_core_tool_bridge_test` (real Qwen turn calls
  an MCP tool: mcpToolStart→execute→mcpToolComplete→complete). PASS.

### NOT PASS — enumerated test FILES that were never authored (the phase-8 gap)
- **TEST-14,20,21,22,23,26,27,41** — `tests/agent/{journal,settings,reviewer,verification,tool_verbosity,migration,model_resolver}_test.rs` — files do not exist.
- **TEST-15,16,17,18,19,37** — `tests/workflow/agent_step*/agent_fanout_test.rs` — files do not exist (workflow `kind:agent` was verified via lib unit tests in waves 2/3, not these integration files).
- **TEST-24,25** — `tests/chat/{agent_core_parity,extension_split}_test.rs` — not authored (parity was proven via the diff of legacy-vs-flag failure sets, not a single file).
- **TEST-36** — `agent-core/tests/deps_boundary.rs` — not authored.
- **TEST-28,29,30,31,33,39** — e2e Playwright specs (`ui/tests/e2e/…`) — not authored; require a Playwright/gallery run.
- **TEST-32** `[negative-perm]` — `ui/tests/e2e/settings/agent-settings-negperm.spec.ts` — not authored. **A10 requires this** (the feature introduces `agent::settings::{read,manage}`): an e2e proving a user LACKING the perm sees no agent-settings UI. This is the mandatory frontend authz proof and the single most important remaining test.

npm run check (ui): FAIL — pre-existing base debt (`check:testid-registry` stale; reproduces on clean HEAD), independent of this feature; a repo-wide `gen:testid-registry` regen is out of scope. tsc + guardrails + colors + the agent-scoped `gate:ui` (0 HIGH) pass.

## Phase-8 blocker (exact)
`lifecycle-check --phase 8` cannot pass until the ~25 enumerated integration + e2e
FILES above are authored and run GREEN — most importantly the A10 `[negative-perm]`
e2e (`TEST-32`) and the happy-path agent e2e — which needs a Playwright/gallery
environment. This is a substantial, separate test-authoring + e2e-run effort; it is
NOT a code defect in the migration (the implementation is complete and its behavior
is verified by the passing set above + the blind audit + fixes in phases 6/7).

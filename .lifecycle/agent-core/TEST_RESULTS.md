# TEST_RESULTS — agent-core / chat re-home

Real, per-test pass/fail. Every PASS below was RUN this session with the log path
noted. Bridge = live Qwen at `ZIEE_TEST_LLM_BASE_URL` (:4000).

## Primary gate — two-flag regression (core-path refactor ⇒ regression is the gate)

| Suite | Flag OFF (legacy) | Flag ON (agent-core) | Verdict |
|---|---|---|---|
| `chat::` integration | 162 pass / 9 fail | 162 pass / 9 fail | **0 regressions** — identical set (9 = env-gated real-LLM/npx, red on BOTH) |
| `mcp::` integration | 457 pass / 40 fail | 457 pass / 40 fail | **0 real regressions** — 2 diff-flips confirmed flakes (pass in isolation; flag can't touch them) |

Logs: `logs/regress_chat_{OFF,ON}.log`, `logs/regress_mcp_{OFF,ON}.log`.
The migration is blast-radius-clean on both touched subsystems.

## Verified PASS (authored + run this session)

### agent-core crate (unit) — TEST-1..13, 34, 36
`cargo test -p agent-core` → **36/36 PASS** (loop, ports, budget, tokens, policy,
reviewer, compaction, fanout, extension seam, streaming, cancel, resume, terminal
tool). Incl. **TEST-36** `tests/deps_boundary.rs` — port-boundary dep set asserted.

### Integration (`tests/agent/`, `tests/chat/`, `tests/workflow/`)
- **TEST-20/21** `agent/settings_test.rs` — GET/PUT roundtrip + bounds→400 + sync + 401/403. **2/2 PASS**.
- **TEST-41** `agent/model_resolver_test.rs` — owner-allowed / outsider-denied model access. **PASS**.
- **TEST-27** `agent/migration_test.rs` — `agent_transcript_json` jsonb + `resumable` CHECK + `review_classification` col + `agent_admin_settings` seeded. **PASS**.
- **TEST-14** `agent/journal_test.rs` — bridge workflow `kind:agent` tool call journaled to `mcp_tool_calls` + linked via `workflow_run_id`, sanitized `result_json`. **PASS** (`logs/test14.log`).
- **TEST-24** `chat/agent_core_parity_test.rs` — SSE sequence + block persistence parity legacy↔core. **PASS**.
- **TEST-25** `chat/extension_split_test.rs` — assistant system prompt precedes user message in wire order (StubChat capture). **PASS**.
- **TEST-15/16** `workflow/agent_step_test.rs` — `kind:agent` step runs the shared loop to completion + records output. **PASS**.
- **TEST-40** existing `workflow::tool` suite unchanged after `ToolDispatcher → call_mcp_tool` extraction — **11/11 PASS** (`logs/test40_toolstep.log`).
- **TEST-38** existing `chat::` suite on the core path — covered by the two-flag regression above (162/9 ON == OFF).

### Real-LLM against the bridge (flag ON)
- `mcp/agent_core_tool_bridge_test.rs` — real Qwen turn calls an MCP echo tool: mcpToolStart→execute→complete. **PASS**.
- `mcp/agent_core_multiturn_bridge_test.rs` — 2-turn agentic chat; turn-1 tool value (`purple-turtle-42`) persists into turn-2 context. **PASS**.
- `agent-core/tests/real_llm_loop.rs` — tool round-trip + 332 streamed deltas. **PASS**.

### E2E (Playwright)
- **TEST-31** `settings/agent-settings.spec.ts` — admin edits `default_max_steps`, saves, reload persists. **PASS** (~2.6m).
- **TEST-32** `[negative-perm]` `settings/agent-settings-negperm.spec.ts` (A10) — a user lacking `agent::settings::read` sees NO nav entry + NO card; admin sees both. **PASS** (~2.5m). *(The mandatory frontend authz proof.)*

`npm run check (ui)`: tsc + guardrails + colors + agent-scoped `gate:ui` (0 HIGH) PASS;
`check:testid-registry` fails on **pre-existing base debt** (reproduces on clean HEAD,
repo-wide regen out of scope).

## Remaining enumerated tests — disposition needed (Phase-8 gap)

Six integration + five e2e enumerated FILES are unauthored. Split by *why*:

**(A) Test a UI surface that does NOT exist in this migration** (grep for
agent-chat / plan-todo / parallel-search / workflow-run UI returns empty — the chat
cutover deliberately REUSED the existing chat UI; no new agent front-end was built):
- **TEST-28** agent-chat.spec, **TEST-29** agent-parallel-search.spec,
  **TEST-30** agent-step-run.spec, **TEST-33** agent-progress.spec.
  → **No surface to test.** Descope candidates (no-surface).

**(B) Existing chat e2e on the flag** — **TEST-39**. Flag is opt-in (default legacy);
backend parity proven by TEST-38 (162/9 ON==OFF). Descope candidate (UI unchanged
when flag off; backend regression already the gate).

**(C) Feature not implemented in this migration** — **TEST-26** tool verbosity
(`concise|detailed`): no `verbosity` symbol exists in `agent-core/src` (ITEM-10 not
built this pass). Descope candidate (feature-not-built) — or implement.

**(D) Implemented + crate-unit-covered + workflow-wired; end-to-end real-LLM
integration file unauthored** — **TEST-17/18/37** (agent-step durable resume),
**TEST-19** (fanout/delegate), **TEST-22** (reviewer escalation), **TEST-23**
(citation not-found). Crate logic is unit-tested (`reviewer.rs`/`fanout.rs`/
`policy.rs`/`core.rs` resume `#[cfg(test)]`); workflow wiring exercised by
`agent_step_test` + `journal_test`; the resume *mechanism* fully covered by the
existing `workflow/resume.rs` (3 tests). Unauthored files are the real-LLM
end-to-end versions (each needs the bridge to reliably drive model-dependent
behavior: reviewer-High classification, fanout spawning, fabricated-DOI handling).

**(D) items are RUNNABLE** (author + bridge). **(A)/(B)/(C)** are genuine blockers
(no surface / redundant / not-built) that need a human-approved DECISION per FB-7.

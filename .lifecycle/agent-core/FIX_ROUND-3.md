# FIX_ROUND-3 — blind convergence + expanded-scope pre-existing sweep

## Part A — Blind multi-angle audit, round 3 (convergence check)

A fresh/blind auditor (diff-only context) reviewed the full agent-core surface
(core.rs, ports.rs, gate.rs, resolver.rs, uniquify.rs, agent_dispatch.rs,
dispatch.rs `call_mcp_tool`, tests/common/mod.rs). Verdict: **no high- or
medium-severity defect; the surface is sound.** The approval matrix, single-use
claim, fail-safe denial read, `is_trusted` name→id→`is_builtin_server_id`
round-trip (correctly excluding code_sandbox/control), raw-uuid accessibility
re-validation, `resolve_bare_tool_server` ambiguity guard, sampling-vs-pooled
split, and orphan-`tool_use` avoidance were all confirmed correct.

Three low items were raised; all previously known and non-defect:
- param footgun (adjacent `Option<String>` review_classification/idempotency_key)
  — both call sites correct; latent (= R2.3 accepted-latent).
- `AgentCoreFlag` `#[must_use]` not firing on `let _ =` — all callers use a
  named binding; discipline-only (= accepted-latent ledger row).
- cancel-bridge coupling — VERIFIED present + correct in-diff at
  `dispatcher.rs:211-223` (chat stop token → crate `CancelToken`). Not a defect.

**New confirmed findings:** 0

→ The agent-core blind audit has **CONVERGED** (round 3 yields 0 new confirmed
agent-core defects).

## Part B — Expanded-scope pre-existing deterministic-bug sweep

Per the expanded directive ("fix any real ERROR/BUG even if pre-existing;
distinguish real deterministic bugs from model/environment flakes"), the OFF
baseline's deterministic (non-real-LLM) failures were root-caused. Results:

### Fixed (5 real deterministic bugs — each red→green re-verified OFF)
1. `tests/path_resolution.rs:153` — compile error (`.embedded.expect()` moved a
   non-Copy field out of a borrow) broke the whole binary → `.as_ref()`.
   red: build error (0 ran) → green: **6 passed**.
2. `tests/memory_mcp/mod.rs` `test_recall_requires_memory_enabled` — test assumed
   memory is disabled-by-default, but the schema **seeds enabled=true**; the
   recall `MEMORY_DISABLED` guard is correct. Test now disables memory first,
   then asserts the guard. red: FAILED → green: ok.
3. `tests/chat/test_single_assistant_message_architecture.rs:357` — parsed the
   messages endpoint as an array; it returns `PaginatedMessages` (object). Extract
   `.messages`. red: 'invalid type: map' → green: ok.
4. `tests/mcp/mcp_streaming_workflow_test.rs:746` — same stale array-vs-object on
   the same endpoint. Extract `.messages`. red: FAILED → green: ok.
5. `tests/mcp/elicitation_mcp_test.rs:236` — `BUILTIN_SUFFIXES` missing `__run_js`
   (js_tool auto-attaches). Added it. red: FAILED → green: ok.

### Reported — needs a product scoping decision (NOT unilaterally fixed)
- **files_mcp write-tool attach gate** (8 `agentic_chat::*` tests). Root cause:
  `attach_files_mcp` is gated on `manifest_available = !files.is_empty()`
  (available_files.rs:292 → file.rs:117), so `create_file`/`edit_file` never
  attach in an EMPTY conversation → the model cannot author the first file; the
  StubChat harness also has no `create_file` plan arm. Pre-existing on main,
  outside the agent-core diff. The fix is a core-chat product-behavior change
  with wide blast radius (would add ~10 file tools to every conversation and
  break other committed assertions, e.g. the elicitation empty-config test). See
  `PREEXISTING_BUGS.md`. Surfaced for the user to greenlight as a separate change.

### Classified model-flaky / environmental (NOT code bugs)
- `tool_call_history::chat_path_tool_call_records_source_chat` — fails at the
  `tool_starts > 0` GUARD the test itself documents as an LLM-miss detector; the
  weak local Qwen didn't emit the tool call.
- `mcp_sampling`, `stdio_transport::test_stdio_list_server_tools`,
  `workflow_mcp::resources_test::resources_read_*`, `elicitation ..._real_llm_*`
  — run-to-run churn between two runs of the SAME code (regress2↔regress3),
  confirming flakiness, not determinism.

**Guardrails held:** no push, default flag unchanged, OFF gains zero new failures
(it LOSES the 4 mcp/chat deterministic ones above by the named fixes — an
improvement, attributed per the expanded gate). Two-flag regression re-run to
confirm.

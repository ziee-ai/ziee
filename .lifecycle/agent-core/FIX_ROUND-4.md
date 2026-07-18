# FIX_ROUND-4 — shared-code (framework-quality) fixes + flake classification

Addresses the two HIGH shared-code findings the audit weights heavily, plus
classification of the two candidate flag-delta failures the prior regression
surfaced.

## Part A — the two HIGH fixes (shared/framework code)

### A1. MODULARITY / §9 DAG — move the MCP-call chokepoint to shared `mcp/`
`chat::agent_host` imported its core MCP-call chokepoint
(`call_mcp_tool`/`resolve_tool_server`/`McpCallScope`/`McpToolCallError`/
`CancelSignal`/`ChatCallCtx`) from `workflow::dispatch` — a lateral chat←workflow
coupling for what is SHARED MCP infra (a DAG inversion: chat depended on the
workflow feature module for its own core path).

**Fix:** created `src-app/server/src/modules/mcp/agent_tool_call.rs` (shared) and
moved the entire chokepoint + the built-in NAME→id map there VERBATIM. Both the
workflow dispatcher and the chat agent host now import it from `mcp/`, not from
each other. The one workflow-owned binding — `impl CancelSignal for
registry::RunHandle` — stays in `workflow::dispatch` (orphan rule). `dispatch.rs`
re-exports the moved items (`pub(crate) use`) for its own internal callers.
Verified: `grep` shows ZERO remaining chat→workflow imports for the chokepoint
(only a doc-comment mentions the workflow twin).

### A2. MAINTAINABILITY + LATENT BUG — de-dup adapters, reconcile `terminal`
`split_tool_name` + `mcp_to_agent_result` were copy-pasted in `chat/resolver.rs`
and `workflow/agent_dispatch.rs` and had DIVERGED: chat computed `terminal` from
the `audience:["user"]` annotation (correct MCP semantics, parity with the MCP
extension's `execute_tool`); workflow **hardcoded `terminal: false`**.

**Fix:** de-duplicated into ONE shared pair in `mcp/agent_tool_call.rs`, unified
on the audience-computed `terminal`. **Workflow's `terminal:false` was a latent
bug** — the workflow agent-step ignored the audience-terminal signal (a
user-audience tool result should end the turn, not trigger another model round).
Now both hosts honor it. Real bug fix, noted here.

**OFF byte-identical:** the chat OFF path uses `execute_tool` (not
`call_mcp_tool`), so the move can't affect it. The `terminal` reconciliation
affects the workflow agent-step equally under both flags (that flag is chat-only),
so it introduces no flag-delta; typical workflow tools carry no `audience:["user"]`
annotation, so the value computes to `false` as before — the change only corrects
the annotated case.

Lib compiles; lib unit tests for the moved code 10/10 (`workflow::dispatch` +
`mcp::agent_tool_call`).

## Part B — the two candidate flag-delta failures: BOTH model-flaky (NOT regressions)

The prior full regression showed 2 mcp_ON-only failures. Confirmed via 3×ON /
3×OFF isolation runs against the bridge:

| Test | ON | OFF | Verdict |
|---|---|---|---|
| `workflow_mcp::workspace_test::t4_workspace_verbs_honor_approval_mode` | **3/3 pass** | 1 fail / 2 pass | FLAKE — the ON path *honors* approval mode (3/3); the failure is on OFF. NO approval-bypass security regression. |
| `mcp::mcp_approval_workflow_test::test_pending_approvals_cancelled_on_new_message` | pass when it completes | pass when it completes | FLAKE — real-LLM stream runaway (6808 frames) → intermittent timeout on both flags. |

Both are real-LLM/bridge tests (t4 gated on `ANTHROPIC_API_KEY`); the weak local
Qwen's nondeterminism produces run-to-run churn. Not deterministic flag-delta.

## Part C — final blind convergence round

A fresh/blind auditor reviewed the full 1203-line refactor diff + the resulting
`mcp/agent_tool_call.rs`, `workflow/dispatch.rs`, `workflow/agent_dispatch.rs`,
`chat/agent_host/{resolver,gate,dispatcher}.rs`, and the agent-core `terminal`
CONSUMER (`agent-core/src/core.rs`). Verdict: **reviewed surface is sound;
0 genuine defects.**

- Claim 1 (move) VERIFIED behavior-preserving: `call_mcp_tool` et al. are
  byte-identical pre/post-move; the uuid-branch accessible-set check, disabled-
  server gate, sampling-vs-pooled selection, run linkage, and `tokio::select!`
  cancel race are all preserved. No remaining lateral chat↔workflow import for the
  path; orphan-rule/visibility/re-export all correct and fully consumed.
- Claim 2 (`terminal`) VERIFIED correct + a genuine fix: confirmed against the
  consumer (`core.rs`) that `terminal` only short-circuits when EVERY tool in the
  round was terminal, and is true only for an `audience:["user"]`-exactly block —
  so the workflow agent-step's behavior changes ONLY for that annotated case,
  where ending the turn is the intended semantics (matching the chat twin). No
  scenario where honoring it breaks workflow wrongly.
- One minor STYLE nit (not a defect): `agent_dispatch.rs:218` called
  `resolve_tool_server` via the workflow re-export rather than directly from
  `mcp/`. **Applied** the cleanup (import from `mcp::agent_tool_call`; removed the
  now-unused `resolve_tool_server` from the `dispatch.rs` re-export). Recompiled
  clean. Behavior-neutral (same fn, different import path).

**New confirmed findings:** 0

→ The shared-code refactor has **CONVERGED** (blind round yields 0 new confirmed
defects). Combined with the round-3 agent-core convergence, the full audit is
converged.

# DECISIONS — sandbox-tool-approval-loop

### DEC-1: What format for a minted (synthesized) tool_use id?
**Resolution:** `format!("call_{}", Uuid::new_v4())` — a `call_`-prefixed UUID.
**Basis:** convention — the `call_` prefix matches the OpenAI tool_call_id convention already
flowing through `ai-providers/src/providers/openai.rs`, and the approved plan specifies it. UUID
guarantees global (hence cross-iteration) uniqueness with no DB round-trip.

### DEC-2: Keep `max_iteration = 10` or raise it for multi-step sandbox tasks?
**Resolution:** Keep 10 (`defaults/models.rs`), no code change.
**Basis:** user — the approved plan (Fix D) keeps 10; a realistic sandbox flow is 3-5 iterations,
it is per-conversation configurable, and the real exhaustion was the approval spin (fixed by
ITEM-1), not the ceiling.

### DEC-3: When a bare tool name is ambiguous (≥2 servers) or unknown, what happens?
**Resolution:** Leave `server_id` empty and fall through to ITEM-1's clear error tool_result +
approval delete. Never guess a server.
**Basis:** codebase/security — mis-dispatching an approved, side-effecting tool (e.g.
`execute_command`) to the wrong server is worse than a visible, actionable failure. The
recovery helper returns `Some` only for unambiguous single-server hits.

### DEC-4: Does code_sandbox stay approval-gated (not auto-approved) after the fix?
**Resolution:** Yes — unchanged. `execute_command` still requires approval under the default
`ManualApprove` mode.
**Basis:** codebase — `code_sandbox_server_id()` is deliberately excluded from
`is_builtin_server_id` (`mcp.rs:229-255`) and `auto_attach_builtin_ids` (`mcp.rs:121-207`); the
existing test `mcp.rs:3183-3188` asserts it. The bug is about executing an *already-approved*
tool, not about changing approval policy.

### DEC-5: Where is server_id recovery applied — at finalization or at approval-insert?
**Resolution:** At finalization in `get_accumulated_content`, consulting a per-message
`tool_name_server_map` stashed by `before_llm_call`.
**Basis:** codebase — finalization is the single point where the tool_use block is created
before persistence, so fixing it there makes the persisted block, classification, approval row,
execution, and tool_result all consistent. The stashed map reproduces the exact advertised tool
set (post gating/filtering/drops) with no extra hot-path DB call, mirroring `tool_use_accumulator`.

### DEC-6: Should the error branches also record the tool_use_id as executed?
**Resolution:** Yes — each error/delete branch pushes `tool_use_id` into the returned
`executed_tool_use_ids` vec (in addition to pushing the error tool_result), mirroring the
success path (`mcp.rs:671`).
**Basis:** convention/codebase — keeps the id marked resolved even before the tool_result is
persisted, closing any window where a caller relying on `executed_tool_use_ids` could re-queue it.

### DEC-7: Add a dedicated `just` gate target for the new tests?
**Resolution:** Yes — add `check-mcp-approval` and append it to `check:`; new integration tests
use the `mcp_approval_loop_` name prefix so one filter selects them.
**Basis:** user — chosen explicitly via the plan-time AskUserQuestion ("Also add a just gate
target").

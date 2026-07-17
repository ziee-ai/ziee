# SOTA_FIDELITY — prior-art invariants the implementation MUST honor

Turns "learn from SOTA" into a gate: each invariant → primary source → ziee adaptation → the proof
TEST-ID (Phase 8 enforces it) + a Phase-6 `prior-art-fidelity` audit angle. `Status` starts `to-verify`.

| INV | Primary source | Invariant | Ziee adaptation | Proof | Status |
|---|---|---|---|---|---|
| INV-1 | DBOS/Temporal/LangGraph | Durable boundary = a **completed** tool call; never mid-call. | journal to `mcp_tool_calls` on completion | TEST-14,18 | to-verify |
| INV-2 | LangGraph/DBOS | Resume re-enters deterministically; completed work served, not re-run. | reload `agent_transcript_json` + continue | TEST-13,18 | to-verify |
| INV-3 | Mastra | Snapshot only at pending/suspend/complete; stream tokens out-of-band. | SSE deltas continuous; snapshot at gate/completion | TEST-17,37 | to-verify |
| INV-4 | Letta | Compaction automatic/core, not a tool; sliding 30/70 escalate; core-memory verbatim. | `CompactionExtension` core tier, `before_model` late | TEST-5,34 | to-verify |
| INV-5 | Codex | Reviewer fail-closed; only approval-needing calls; Low/High/Critical. | `reviewer.rs` subagent | TEST-11,22 | to-verify |
| INV-6 | Codex | `max_depth=1`, `max_threads=6`; subagent returns a summary. | `SubagentLimits`; child omits `delegate` | TEST-6,19 | to-verify |
| INV-7 | Goose | Coarse events; tool requests inside messages; provider streaming-first. | `AgentEvent`; `ai-providers` streaming-first | TEST-2,3 | to-verify |
| INV-8 | ports/crate boundary | The core crate has NO app/server dependency. | `agent-core` deps = {`ai-providers`,`ziee-core`,`ziee-identity`}, EXCLUDES the `ziee` server crate | TEST-36 | to-verify |
| INV-9 | ziee `ChatExtension` generalization | One extension model, host-agnostic — same extension runs in every host. | `AgentExtension` seam | TEST-34 (+ chat/workflow both drive it) | to-verify |
| INV-10 | Codex escalation | Escalate to a human only when the reviewer is unsure (`High`). | `High` → durable `elicit` gate | TEST-22 | to-verify |
| INV-11 | LangGraph `@task` / Vercel `stepId` | In-flight side-effecting call on resume needs an idempotency key. | `<run_id>:<turn>:<ordinal>` in the MCP call context | TEST-13 | to-verify |

## DEVIATIONS (intentional)
- **DEV-1** — NOT Mastra's "agent turn = workflow run": the agent loop is an activity, the workflow step is the durable boundary (Temporal nesting; ports give per-host durability tiers). DESIGN §5.5.
- **DEV-2** — Codex per-call bwrap SandboxMode enforcement descoped (DEC-2); ship the policy/approval half.
- **DEV-3** — Goose's concrete session/gate → ziee makes transcript/gate/policy/model-resolve **traits** (ports).
- **DEV-4** — LangGraph "re-run the node" → ziee reloads the persisted transcript (completed tool_results in it); only an in-flight call re-runs (INV-11).
- **DEV-5 (SDK)** — the agent core is a **ziee crate, not an SDK crate** (ziee-only; may name domain; no N9). It deps SDK crates (`ziee-core`/`ziee-identity`) but is not domain-neutral.

## Gating
- **Phase 8:** every `Proof` TEST-ID must PASS.
- **Phase 6:** a `prior-art-fidelity` audit angle reconciles each core-invariant hunk against this ledger.
- **Phase 5:** reference-first — re-read the primary-source extract + DESIGN_REFERENCE before each subsystem.

# DECISIONS — workflow-kind-agent

The two headline product forks were resolved WITH the human this session (recorded as DEC-0a/0b);
the rest resolve by codebase precedent / existing convention. Every decision below is resolved — no
open markers remain.

### DEC-0a: Authoring scope — focused agent wizard, general builder, or run-UX-only?
**Resolution:** General visual workflow builder (add/edit all 6 step kinds), with the AGENT step given
the deep-friendly treatment and the other 5 kinds a functional schema-driven form.
**Basis:** user — explicit AskUserQuestion pick this session ("kind:agent isn't usable without a way to
author workflows, currently YAML-only").

### DEC-0b: Run-UX depth — friendly domain-language timeline or minimal collapse-fix?
**Resolution:** Friendly domain-language activity timeline (typed append stream + editorial-row renderer
+ progressive disclosure + inline gate). Non-negotiable core.
**Basis:** user — explicit pick this session; it is the whole point per the friendly-agent-surface research.

### DEC-1: Which permission gates create vs edit-definition?
**Resolution:** `POST /workflows` (create) gates `WorkflowsInstall` (`workflows::install`); `PUT
/workflows/{id}/definition` (edit) gates `WorkflowsManage` (`workflows::manage`) + owner check.
`validate-def` gates `WorkflowsRead`.
**Basis:** codebase — mirrors `import` (WorkflowsInstall, dev.rs) and `update_user_workflow`
(WorkflowsManage, handlers/mod.rs:187). No NEW permission → no A9/A10.

### DEC-2: Builder v1 — user-scope only, or also admin system-scope?
**Resolution:** User-scope only. System/admin workflows keep coming via `install-from-hub`.
**Basis:** convention — the import create path this mirrors is user-scope; admin system authoring is a
separate hub flow out of this feature's scope.

### DEC-3: Step model — linear ordered list or branching graph?
**Resolution:** Linear ordered step list; cross-step data flow is by template references (ITEM-10
ref-insert), matching the runner's existing ref-based dataflow. No visual branching/conditionals.
**Basis:** codebase — the runner resolves refs (DAG via `steps.x.output` refs), not an authored graph;
a graph editor is out of scope (and a descope candidate's neighbour).

### DEC-4: Which step kinds are addable, and how are they labelled?
**Resolution:** All 6 (`Agent`/`Llm`/`LlmMap`/`Sandbox`/`Elicit`/`Tool`). In the kind picker the agent
kind is labelled in domain language ("AI assistant task"); the others keep their functional names. The
builder entry affordance is "New workflow"; the agent-step form header is "What should the assistant do?".
**Basis:** user (friendly-agent-surface framing) + convention.

### DEC-5: Effort dial → `max_steps` preset values — fixed constants or admin-configurable? (configurable-settings rule)
**Resolution:** **Fixed UI presets** Quick=10 / Balanced=30 / Thorough=60 (Balanced = the existing
`default_agent_max_steps` = 30), with an advanced-disclosure exact `InputNumber` escape hatch.
**Basis:** codebase — the *effective* limit is ALREADY admin-configurable: `agent_admin_settings` clamps
`max_steps` at runtime (agent_dispatch.rs:697-725). The presets are UI sugar over an
already-governed cap, so a second settings row would be redundant; a `Limits`-style const table
(`EFFORT_PRESETS`) keeps them promotable. No new admin settings row.

### DEC-6: `output_format` default in the agent form?
**Resolution:** Text (Structured is opt-in via the Segmented).
**Basis:** codebase — `OutputFormat::default() == Text` (validate.rs).

### DEC-7: Agent-activity persistence + retention — new settings row, or ride the run lifecycle? (configurable-settings rule)
**Resolution:** No new retention tunable and no new settings row. The append history lives in the
existing `workflow_runs.step_logs_json`, so it is bounded by and deleted with its parent run (whatever
run retention already governs). To bound intra-run growth, cap the persisted agent-activity entries per
step at a **fixed `Limits`-style constant** `AGENT_ACTIVITY_MAX_ENTRIES = 500` (keep most-recent,
oldest dropped) and cap each entry's `detail`/tool-args at **16 KiB** and `title` at 512 B.
**Basis:** convention — mirrors the chat/MCP tool-result caps (16 KiB args / 1 MiB result, base64
stripping) which are fixed safety bounds, not operator knobs. Promotable via the const if ever needed.

### DEC-8: Capability picker source + presentation?
**Resolution:** `Stores.McpServer.servers` (the accessible enabled set), presented by `display_name`
as capabilities; selection maps to the `servers: string[]` id allow-list. Empty state ("no tools
available") when the user has no accessible servers.
**Basis:** codebase — audit Q3 (`mcp/stores/McpServer.store.ts`; sanctioned cross-module proxy).

### DEC-9: Validation UX — block save on errors?
**Resolution:** Block save on `errors` (surface them inline via `validate-def`); allow save with only
`warnings`. Live-validate on edit (debounced), not just on save.
**Basis:** convention — `ImportWorkflowDialog` renders the same validation alert before install.

### DEC-10: Is `ProgressKind::AgentActivity` shared with the chat agent host, or workflow-only?
**Resolution:** Workflow-only (emitted by the workflow `WorkflowEventSink`; the chat agent host keeps
its own chat-message rendering). The variant lives in the workflow `events.rs`.
**Basis:** codebase/scope — the two hosts assemble AgentCore from the same ports but render
independently; this feature touches only the workflow surface.

### DEC-11: Descope now, or hold candidates?
**Resolution:** Nothing descoped now — all 16 items are TEST-covered and in scope. The pre-identified
descope candidates (ITEM-10 ref-insert; the non-agent kinds' form polish) will be escalated for
**human approval** and recorded as `DESCOPED: ITEM-N … [approved: …]` ONLY if Phase-5 convergence
demands it. The agent-step friendliness (ITEM-9) + the run timeline (ITEM-11/12/13) are never descope
candidates (the feature's core).
**Basis:** user — the confirmed descope discipline ("write it up, don't silently cut").

### DEC-12: Editing a workflow — preserve id, or new id per save?
**Resolution:** Preserve the workflow id (in-place `update_definition`), so run history + any `wf_<slug>`
tool binding survive an edit.
**Basis:** codebase — audit Q5 (import's delete+insert mints a new id; a re-save must not).

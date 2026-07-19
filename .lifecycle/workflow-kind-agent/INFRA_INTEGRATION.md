# INFRA_INTEGRATION — workflow-kind-agent (the three mandatory Phase-5 walks)

## 1. User-experience walk (how a real user encounters each item)

- **Author (builder):** user opens `/settings/workflows` → "New workflow" → builder. Adds steps from a
  kind picker; for the agent kind gets the friendly form ("What should the assistant do?", capability
  picker, effort, output). Live validation shows problems as they edit. Save → the workflow appears in
  the list and is immediately runnable. Later: "Edit" from the drawer reopens the SAME workflow
  (id preserved) with its steps loaded; change + save persists.
- **Run (timeline):** user runs a workflow containing an agent step (existing Run dialog, unchanged).
  The agent step now shows a **followable, accreting** activity stream in domain language ("Searching
  the literature… Reading 3 papers… Drafting a summary") with per-row "Show details" and a status pill,
  instead of one collapsing log line. If the agent hits a human gate, it renders inline and is
  answerable; the run resumes.
- **Failure surfaces the user must see:** a def that fails validation blocks save with itemised errors
  (not a silent boolean); a save that hits a server error shows a message; an agent step that fails
  shows the terminal error row in the timeline (existing step error path preserved).

## 2. Infrastructure-integration walk (every subsystem touched)

- **Workflow runner / dispatch:** ITEM-5 edits ONLY `WorkflowEventSink`'s progress mapping in
  `agent_dispatch.rs`; the dispatch/resume/gate/cancel/budget logic (`:639-1039`) is untouched. The new
  `AgentActivity` is derived from the SAME `AgentEvent`s already observed — no new event source.
  Constraint: the sink runs on the run's async task; the durable append writer must be
  fire-and-forget-safe (a DB hiccup must not fail the run) — mirror the existing `persist_step_logs`
  spawn pattern, never `?` the append into the run result.
- **MCP tool-call + approval:** the agent step's tools come from the `servers` allow-list resolved via
  the existing `McpToolProvider` port; approval/reviewer/sandbox all come from `agent_admin_settings`
  (unchanged). The builder only WRITES the `servers` name list — it does not touch approval. Constraint:
  a `servers` entry naming a server the acting user can't access must degrade gracefully (the provider
  simply won't expose it); `validate-def` should WARN on an unknown server name, not hard-fail (names
  are resolved at run time per-user).
- **Permissions:** reuses `workflows::install/manage/execute` + owner checks (DEC-1). No new perm, no
  grant migration. `validate-def` = `workflows::read`.
- **Sync (notify-and-refetch):** ITEM-2/3 create/update MUST `sync_publish` the `Workflow`/`UserWorkflow`
  entity (owner audience) after the DB write — mirror `emit_user_workflow` in the existing
  `update_user_workflow`. The builder store subscribes to `sync:workflow` so a cross-device edit
  refetches. The run timeline already consumes the run SSE; the new `AgentActivity` frame rides the
  SAME `SSEStepProgress` event stream (no new SSE endpoint).
- **Streaming:** the new `ProgressKind::AgentActivity` variant is additive on the existing
  `SSEStepProgress` frame; every `match` on `ProgressKind` (compiler-enumerated) gains an arm.
- **OpenAPI:** new endpoints + `WorkflowDef`/`StepConfig`/`ProgressKind::AgentActivity` → regen BOTH
  binaries; the `types_ts_parity` golden test guards it.
- **Settings/notifications:** none touched (no new settings row per DEC-5/7; no notifications).
- **Desktop:** shared `ui/` modules (no mirror) — the builder + timeline ship to desktop for free;
  only the desktop api-client regen + a possible coverage-classifier string (ITEM-15).

## 3. Entity-lifecycle walk (add / remove / mutate / access-loss — BOTH local + sync paths)

- **Workflow being edited in the builder:**
  - *deleted locally* (another tab / the drawer Delete): the builder's `sync:workflow` handler must
    detect the open workflow's id vanished → show "this workflow was deleted" and navigate back to the
    list (don't let Save 404 silently). *deleted via sync (other device):* same handler covers it.
  - *mutated* (metadata rename elsewhere): builder holds the DEFINITION; a metadata-only change doesn't
    conflict, but on save the builder writes def only (not metadata), so no clobber.
- **Run open in the timeline:**
  - *cancelled* (local Run-cancel button OR sync): the run SSE emits terminal status → the timeline
    stops the live rows and shows the cancelled state (existing run-status path; the new
    `agentActivity[]` is read-only history, so it simply freezes).
  - *deleted* (run pruned / conversation deleted cascade): the run row + its `step_logs_json` (holding
    the activity) cascade together — no orphan; the timeline view, if open, refetches empty → shows a
    "run not found" state (existing behavior).
- **MCP server referenced by an agent step:**
  - *removed/disabled* while in the `servers` allow-list: at author time `validate-def` WARNS (unknown
    name); at run time the tool is simply absent (graceful). The capability picker reads live
    `Stores.McpServer.servers`, which already refetches on `sync:mcp_server`, so the form reflects
    add/remove without reload.
- **Accessible-server set changes while the agent form is open:** `Stores.McpServer` syncs; the
  MultiSelect options update reactively. A previously-selected server that lost access stays in the
  written `servers` list (a name), but is shown as unavailable and validate-def warns.

**Convergence note:** every "what happens when X is deleted?" above is proven by RUNNING it in the
Phase-8 e2e (builder-edit delete-while-open; timeline run-cancel), not inferred.

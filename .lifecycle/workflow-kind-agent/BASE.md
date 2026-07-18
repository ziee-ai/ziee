# BASE — conflict-surface scoping (workflow-kind-agent)

**Branch base is NOT `origin/main`.** This branch is **stacked on `feat/agent-core`**
(`feat/workflow-kind-agent` @ `dc5f2fb00`, forked from `feat/agent-core @ dc5f2fb00`). The
kind:agent backend host + `StepConfig::Agent` + the agent-core migrations already live on the base,
so the conflict surface is computed against `feat/agent-core`, not `origin/main`. Do NOT rebase onto
main or re-add anything agent-core already shipped.

## Migrations — NONE added by this feature
- Migrations here are **module-local + date-numbered**. Highest in the workflow module:
  `202607170100_workflow_agent_step.sql`. Highest agent-core-added elsewhere: `202607170105`
  (mcp) / `202607160100` (agent admin settings). If a migration were ever forced, the next free
  number is `202607180001+` (today is 2026-07-18).
- **This feature adds no migration.** The editable source of truth is the on-disk `workflow.yaml`
  bundle (`workflows.extracted_path`); the builder re-materializes + recompiles that bundle and
  updates the existing `compiled_ir_json` column. The friendly agent activity reuses the existing
  `workflow_runs.step_logs_json` append channel (structured entries) — no schema change. No new
  permission (reuses `workflows::install` / `workflows::manage` / `workflows::execute`), so no grant
  migration and no A9/A10 authz gate.

## Files this branch will edit that the base also owns (all inside the workflow module)
- `src-app/server/src/modules/workflow/{routes,handlers/mod,models,repository,validate,cost}.rs`
  — additive (new endpoints/types/fns); no signature change to existing handlers.
- `src-app/server/src/modules/workflow/agent_dispatch.rs` — the ONLY edit to an agent-core-authored
  file: the `WorkflowEventSink` progress mapping (`:343-398`). Additive to the activity shape; the
  dispatch/resume/gate logic is untouched. Watch for churn if agent-core revises `agent_dispatch.rs`.
- `src-app/ui/src/modules/workflow/**` + `src-app/desktop/ui/**` — mostly new files (builder/, run/);
  edits to `WorkflowRunProgressView.tsx`, `WorkflowRun.store.ts`, `module.tsx`,
  `sse/runProgressClient.ts`, `types.ts`.

## OpenAPI regen — IMPLIED (both binaries)
New endpoints (`GET/PUT /workflows/{id}/definition`, `POST /workflows`, `POST /workflows/validate-def`)
+ newly-exposed `WorkflowDef`/`StepDef`/`StepConfig`/`AgentActivity` types ⇒ `just openapi-regen` must
run for BOTH `ui/` and `desktop/ui/`; the `emit_ts` golden-parity test must stay green (ITEM-14).

## Merge-gate note
Because the base is `feat/agent-core`, the eventual merge-gate must diff against `feat/agent-core`
(merge-base), never `origin/main` — else the whole agent-core surface reads as this branch's diff.

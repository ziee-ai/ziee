# TESTS — workflow-kind-agent (every ITEM ↔ ≥1 TEST)

Tiers mirror the codebase: **unit** = in-source `#[cfg(test)]` (Rust) or a `.store.ts`/pure-module
vitest (FE); **integration** = `src-app/server/tests/workflow/*.rs` (Postgres + TestServer);
**e2e** = `src-app/ui/tests/e2e/workflows/*.spec.ts` (real backend through the UI). No new permission
is introduced (reuses `workflows::install/manage/execute`), so **no `[negative-perm]` spec is required**
(A10 N/A); the existing `admin-page-gating.spec.ts` already covers the workflow perm surface.

## Backend

- **TEST-1** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/workflow/builder_crud_test.rs` — asserts: `GET /api/workflows/{id}/definition` returns the editable `WorkflowDef` (steps+inputs) parsed from the bundle; a cross-user id → **404**; unauth → 401.
- **TEST-2** (tier: integration) [covers: ITEM-2] file: `src-app/server/tests/workflow/builder_crud_test.rs` — asserts: `POST /api/workflows` with a `WorkflowDef` body creates a user-scope row (materialize→validate→compile→insert), the row is listable and **runnable** (a subsequent run reaches `completed`); a def failing validation → **422** with structured errors and NO row created.
- **TEST-3** (tier: integration) [covers: ITEM-3] file: `src-app/server/tests/workflow/builder_crud_test.rs` — asserts: `PUT /api/workflows/{id}/definition` replaces steps/inputs **in place — the workflow id is unchanged** and a pre-existing run row still FK-resolves to it; recompiled IR reflects the edit; non-owner → **403**, missing → **404**; the superseded bundle dir is removed.
- **TEST-4** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/server/src/modules/workflow/handlers/dev.rs` — asserts: the `def_to_bundle` helper serializes a `WorkflowDef` → a `workflow.yaml` dir that `pack_workspace_dir` packs and round-trips back to an equal `WorkflowDef` (materialize fidelity).
- **TEST-5** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/workflow/validate_def_test.rs` — asserts: `POST /api/workflows/validate-def` returns `{errors:[], warnings, cost_estimate}` for a valid def; a def with a bad step-ref/kind → non-empty `errors` (200, not a hard fail); an over-cap/invalid field → the matching structured finding.
- **TEST-6** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/workflow/events.rs` — asserts: `ProgressKind::AgentActivity{seq,kind,tool,title,detail,status}` serde round-trips under the `#[serde(tag="type")]` discriminator (`type:"agent_activity"`), and every existing `ProgressKind` variant still round-trips (no regression).
- **TEST-7** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/workflow/agent_dispatch.rs` — asserts: the `WorkflowEventSink` maps each `AgentEvent` (tool-use → `tool_call`, assistant text → `message`, gate → `gate`, compaction → `compaction`) to a distinct `AgentActivity` with a **monotonically increasing `seq`** (no collapse to one entry) and a domain-safe `title`.
- **TEST-8** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/workflow/agent_activity_test.rs` — asserts: running a `kind:agent` step persists an **append-style** history to `step_logs_json` (≥N entries, seq-ordered), and re-fetching the run (snapshot) **replays the full history**, not just the last line; the durable stream survives a simulated resume.

## Frontend — builder

- **TEST-9** (tier: unit) [covers: ITEM-6] file: `src-app/ui/src/modules/workflow/stores/WorkflowBuilder.store.ts` — asserts: the builder store (defineLocalStore) loads a `WorkflowDef`, add/edit/reorder/delete-step mutate the working def, sets the `dirty` flag, and serializes back to a valid `WorkflowDef` payload; validation results are stored from `validate-def`.
- **TEST-10** (tier: e2e) [covers: ITEM-6, ITEM-7] file: `src-app/ui/tests/e2e/workflows/builder-create.spec.ts` — asserts: from `/settings/workflows`, "New workflow" opens the builder; the user adds steps, reorders (dragTo), edits a title, saves; the new workflow appears in the list and its detail drawer shows the steps.
- **TEST-11** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/workflows/builder-edit.spec.ts` — asserts: "Edit" from the `WorkflowDetailDrawer` opens the builder pre-loaded with the existing def; changing a step + save persists (reopen shows the change) and the workflow id/route is unchanged (edit-in-place, not a new workflow).
- **TEST-12** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/tests/e2e/workflows/builder-step-kinds.spec.ts` — asserts: the add-step kind picker offers all 6 kinds; adding a Tool + an Llm step renders their schema-driven config forms (typed fields, not a raw-JSON box), invalid input surfaces inline validation, and a valid config saves.
- **TEST-13** (tier: unit) [covers: ITEM-8] file: `src-app/ui/src/modules/workflow/components/builder/stepForms.ts` — asserts: the per-kind zod-schema builder produces the right typed fields per `StepConfig` kind and rejects out-of-range/missing required fields.
- **TEST-14** (tier: e2e) [covers: ITEM-9] file: `src-app/ui/tests/e2e/workflows/builder-agent-step.spec.ts` — asserts: the AGENT step friendly form — "What should the assistant do?" (instructions), a capability `MultiSelect` populated from accessible MCP servers, an effort `Segmented` (Quick/Balanced/Thorough), an output `Segmented` (Text/Structured), and the "what this task will do" read-back — configures an agent step in domain language (no `run_js`/`max_steps`/server-id jargon on the calm surface); advanced Accordion exposes the system directive; save creates a runnable agent workflow.
- **TEST-15** (tier: unit) [covers: ITEM-9] file: `src-app/ui/src/modules/workflow/components/builder/AgentStepForm.tsx` — asserts: the effort `Segmented` maps Quick/Balanced/Thorough → the correct `max_steps` values; the capability multiselect maps selected `display_name`s → `servers` string ids; the plain-English read-back reflects the current config.
- **TEST-16** (tier: e2e) [covers: ITEM-10] file: `src-app/ui/tests/e2e/workflows/builder-ref-insert.spec.ts` — asserts: the ref-insert menu on a step field lists workflow inputs + prior-step outputs (with type hints) and inserts the correct reference token into the field.
- **TEST-17** (tier: unit) [covers: ITEM-10] file: `src-app/ui/src/modules/workflow/components/builder/RefInsertMenu.tsx` — asserts: given a compiled-IR input/step list, the helper enumerates valid references for the current step (only PRIOR steps) and produces the correct insert token.

## Frontend — friendly run timeline

- **TEST-18** (tier: unit) [covers: ITEM-11] file: `src-app/ui/src/modules/workflow/components/run/activityDescriptors.ts` — asserts: the descriptor registry maps known tool ids (`web_search`→"Searching the web", `literature_search`→"Searching the literature", `code_sandbox`→"Running code") and returns a sensible generic fallback for an unknown tool.
- **TEST-19** (tier: unit) [covers: ITEM-13] file: `src-app/ui/src/modules/workflow/stores/WorkflowRun.store.ts` — asserts: an incoming `AgentActivity` frame is **appended** to the step's `agentActivity[]` and **deduped by `seq`** (not overwritten like `tracks`); snapshot rehydrate array-merges persisted history; non-agent `tracks` behavior is unchanged.
- **TEST-20** (tier: e2e) [covers: ITEM-12, ITEM-13] file: `src-app/ui/tests/e2e/workflows/agent-step-timeline.spec.ts` — asserts: running an agent-step workflow shows a **scrolling domain-language activity timeline** (multiple accreting rows with status pills — NOT a single collapsing line), each row's "Show details" progressive disclosure reveals the underlying tool/output, and a human gate/approval renders inline and is answerable; the timeline reflows at 390px.

## Cross-cutting

- **TEST-21** (tier: unit) [covers: ITEM-14] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the `types_ts_parity` golden test stays green after regen, and `WorkflowDef`/`StepDef`/`StepConfig`/`OutputFormat`/`InputDef` + `ProgressKind::AgentActivity` are present in the regenerated `types.ts` (types now exposed to the FE).
- **TEST-22** (tier: integration) [covers: ITEM-15] file: `src-app/desktop/tauri/tests/build_smoke.rs` — asserts: the desktop-embedded server crate compiles with the new endpoints/types (workflows ship on desktop unchanged), and the desktop `api-client` regen is parity-clean (committed == fresh regen). [Build/compile verification; run in Phase 8.]
- **TEST-23** (tier: e2e) [covers: ITEM-16] file: `src-app/ui/tests/e2e/workflows/builder-gallery.spec.ts` — asserts: via the gallery/gate:ui surfaces, the builder (empty / populated 4-step incl. agent / **390px** / validation-error), the agent friendly form, and the run timeline (running / gate-open / completed) each render crash-free (0 gating runtime-health HIGH) for the design-critic + Layer-A/axe passes.

## Coverage summary (bipartite check)
ITEM-1→T1 · ITEM-2→T2,T4 · ITEM-3→T3,T4 · ITEM-4→T5 · ITEM-5→T6,T7,T8 · ITEM-6→T9,T10 · ITEM-7→T10,T11 ·
ITEM-8→T12,T13 · ITEM-9→T14,T15 · ITEM-10→T16,T17 · ITEM-11→T18 · ITEM-12→T20 · ITEM-13→T19,T20 ·
ITEM-14→T21 · ITEM-15→T22 · ITEM-16→T23. Every ITEM covered; every UI item (7,8,9,10,12,16) has a
`tier: e2e`.

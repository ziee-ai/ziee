# PLAN — workflow-kind-agent (make `kind: agent` a real, usable, friendly workflow step)

**Mode:** ITERATION — the `kind:agent` BACKEND host already exists on `feat/agent-core`
(`agent_dispatch.rs` drives `AgentCore::run`; `StepConfig::Agent` is wired through
type_infer/cost/compiled/validate/ref_check/runner/startup_sweep). This feature builds the
**user-facing surface** that makes it (and every other step kind) authorable + legible.

**Product scope (decided with the human, this session):**
- **Authoring = a general visual workflow builder** — add/edit ANY of the 6 step kinds
  (Agent/Llm/LlmMap/Sandbox/Elicit/Tool), order steps, wire inputs→outputs, save. `agent` is one
  selectable kind and gets the deeply-friendly authoring treatment; the other kinds get a
  functional schema-driven config form.
- **Run UX = a friendly domain-language activity timeline** — fix the single-track collapse and
  render an agent step's live work as a scrolling domain-language timeline (editorial rows,
  progressive disclosure, gate/approval inline), per the friendly-agent-surface research handoff.

**Non-goals (this round):** no branching/conditionals in the builder (linear step list only, matching
today's runner); no per-step model override / per-step token budget / per-tool allow-list added to
`StepConfig::Agent` (server-name tool granularity + admin-global caps stay as-is — DEC in Phase 4);
no new permission (reuses `workflows::manage` / `workflows::install` / `workflows::execute`).

---

## Items

### Backend — editable-definition CRUD + friendly agent activity
- **ITEM-1**: `GET /api/workflows/{id}/definition` — return the **editable** `WorkflowDef` (parse
  `workflow.yaml` from the row's `extracted_path`) as JSON. Owner-scoped (cross-user → 404). This is
  what exposes `WorkflowDef`/`StepDef`/`StepConfig`/`OutputFormat`/`InputDef` to OpenAPI → FE types
  (they already derive `JsonSchema`; today the FE only ever sees `compiled_ir_json`).
- **ITEM-2**: `POST /api/workflows` — create a user-scope workflow from a posted `WorkflowDef` JSON.
  Server-side: serialize def → `workflow.yaml` in a fresh extracted bundle dir → `validate_workflow`
  → compile IR → `repository::create` (sha256/size/file_count/entry_point set as `import` does) →
  `sync_publish`. This is the create path deliberately absent today (`routes.rs:9-13`), now added
  because the builder needs a non-tarball source of truth. Gated `WorkflowsInstall`.
- **ITEM-3**: `PUT /api/workflows/{id}/definition` — replace an existing user-scope workflow's
  steps/inputs from a posted `WorkflowDef`: re-materialize the bundle at `extracted_path`, re-validate,
  recompile IR, `repository::update`-equivalent for def+ir+sha, `sync_publish`. Owner-scoped 404/403.
  Distinct from the metadata-only `PUT /api/workflows/{id}` (`UpdateWorkflow`).
- **ITEM-4**: `POST /api/workflows/validate-def` — validate a posted `WorkflowDef` JSON (not a
  tarball) → structured `{errors[], warnings[], cost_estimate}` for live inline validation in the
  builder before save. Thin wrapper over the existing `validate_workflow` + `cost` dry-run.
- **ITEM-5**: Friendly agent-activity stream — replace the single-track `"agent"` `ProgressKind::Log`
  collapse in `WorkflowEventSink` (`agent_dispatch.rs:343-398`) with **typed, append-style**
  `AgentActivity { seq, kind, tool?, title, detail?, status }` entries (kind ∈
  `thinking|tool_call|tool_result|message|gate|compaction`), appended to the run's existing
  `step_logs_json` channel (structured entries — NO new column/migration) so a reopened/resumed run
  replays full history instead of the last line only. Emitted through the existing progress SSE.

### Frontend — the builder
- **ITEM-6**: FE `WorkflowDef`/`StepDef`/`StepConfig` types (from OpenAPI regen) + a per-instance
  builder store (`WorkflowBuilder.store.ts`, `defineLocalStore`) holding the editable def, dirty
  flag, selected-step, and live validation results.
- **ITEM-7**: Workflow **builder** surface (page + entry affordances) — ordered step list, add-step
  (kind picker), drag-reorder, delete step, per-step config panel, workflow-inputs editor, a live
  validation/cost panel, Save (create via ITEM-2 / update via ITEM-3). Entry points: "New workflow"
  on `WorkflowsList`, "Edit" on `WorkflowDetailDrawer`.
- **ITEM-8**: Per-kind step-config forms (functional, schema-driven, kit `Field`) for
  Llm / LlmMap / Sandbox / Elicit / Tool.
- **ITEM-9**: Agent-step **friendly** config form — domain language: "What should the assistant do?"
  (→ `prompt`), a capability/tool picker (multi-select over the user's accessible MCP servers, shown
  as capabilities not server ids → `servers`), an effort dial (`max_steps`, Quick↔Thorough), output
  (Text↔Structured → `output_format`), advanced disclosure (system directive → `system`; exact
  `max_steps`). Plus a plain-English "what this task will do" read-back (show-then-act).
- **ITEM-10**: Step input/output wiring helper — a ref-insert menu that inserts references to
  workflow inputs + prior step outputs into a step's template fields, with type hints from the
  compiled IR's `InferredType`. (Not a graph editor — a linear ref picker.)

### Frontend — the friendly run timeline
- **ITEM-11**: Activity-descriptor registry — tool id → domain-language activity phrase
  (`web_search`→"Searching the web", `literature_search`→"Searching the literature",
  `code_sandbox`→"Running code", …) with a generic fallback. Mirrors the handoff's F1 descriptor map.
- **ITEM-12**: Agent-activity **timeline** renderer in `WorkflowRunProgressView` — render ITEM-5's
  structured activity as a scrolling domain-language timeline (editorial rows: one line per activity,
  right-aligned status pill, "Show details"/tool-args/output progressive disclosure); gate/approval/
  reviewer events surface inline (reuse `WorkflowElicitForm`). Kit: Card/Badge/Accordion/Button/
  ToolStatusIcon/Separator.
- **ITEM-13**: Run-store handling — `WorkflowRun.store.ts` + `sse/runProgressClient.ts` consume the
  new agent-activity entries **append-style (keyed by `seq`)**, not the current overwrite
  (`s.tracks[id] = t`), so the timeline accretes.

### Cross-cutting
- **ITEM-14**: `just openapi-regen` — regenerate BOTH `ui/` and `desktop/ui/` (new endpoints +
  `WorkflowDef`/`StepConfig`/`AgentActivity` types); golden `types.ts` parity test stays green.
- **ITEM-15**: Desktop parity — builder surface + timeline mirrored/registered in
  `src-app/desktop/ui`; verify the desktop-embedded server still builds (`CORE_MODULE_BLOCKLIST`
  unaffected — workflows already ship on desktop).
- **ITEM-16**: Gallery coverage + state matrix — add builder (empty / populated / **390px mobile** /
  validation-error), the agent friendly form, and the run timeline (running / gate-open / completed)
  as gallery surfaces so `gate:ui` (runtime-health + Layer A/axe + narrow-viewport) covers them.

---

## Files to touch

**Backend (all under `src-app/server/src/modules/workflow/`)**
- `routes.rs` — add the 4 new routes (ITEM-1..4).
- `handlers/mod.rs` — `get_workflow_definition`, `create_user_workflow`, `update_user_workflow_definition`,
  `validate_workflow_def` (+ `_docs`), mirroring `import` / `get_user_workflow` / `update_user_workflow`.
- `handlers/dev.rs` — reuse/extract the bundle-materialize helper from `run_from_workspace`/workspace-save
  (`:906-950`) into a shared `def_to_bundle(def) -> extracted_path` used by ITEM-2/3.
- `models.rs` — request/response types (`CreateWorkflowDefRequest`, `ValidateDefResponse`, etc.);
  re-export `WorkflowDef` for OpenAPI.
- `agent_dispatch.rs` — ITEM-5 `WorkflowEventSink` rewrite (`:343-398`) + the `AgentActivity` type.
- `repository.rs` — a `update_definition` fn (bundle path + ir + sha) alongside the metadata `update`.
- `validate.rs` / `cost.rs` — a `validate_def(&WorkflowDef)` + `estimate(def)` entry that skips YAML parse
  (ITEM-4) — reuse the existing pass, don't fork it.

**Frontend (`src-app/ui/src/modules/workflow/`, mirrored into `src-app/desktop/ui/`)**
- `types.ts` — surface the regen'd `WorkflowDef`/`StepDef`/`StepConfig` + builder view-model types.
- `stores/WorkflowBuilder.store.ts` (new), `stores/WorkflowRun.store.ts` (ITEM-13 edit).
- `components/builder/` (new): `WorkflowBuilderPage.tsx`, `StepList.tsx`, `AddStepMenu.tsx`,
  `StepConfigPanel.tsx`, `AgentStepForm.tsx` (ITEM-9), `{Llm,LlmMap,Sandbox,Elicit,Tool}StepForm.tsx`,
  `WorkflowInputsEditor.tsx`, `RefInsertMenu.tsx` (ITEM-10), `BuilderValidationPanel.tsx`.
- `components/run/AgentActivityTimeline.tsx` (new, ITEM-12), `activityDescriptors.ts` (ITEM-11),
  `WorkflowRunProgressView.tsx` (edit to mount the timeline for agent steps).
- `module.tsx` — register the builder route + `WorkflowsList`/`WorkflowDetailDrawer` "New/Edit" affordances.
- `sse/runProgressClient.ts` — parse the new agent-activity frames.
- `src/dev/gallery/` (+ desktop mirror) — ITEM-16 gallery surfaces.

**Generated (do not hand-edit):** `api-client/types.ts`, `openapi/openapi.json` (both workspaces) via ITEM-14.

---

## Patterns to follow (closest existing module to mirror — hard rule)

- **New editable-def endpoints (ITEM-1..4):** mirror `handlers/mod.rs::import` (bundle materialize +
  validate + compile + `repository::create`) and `update_user_workflow` (owner-scope 403/404 + `sync_publish`).
  The bundle-from-not-a-tarball precedent is `handlers/dev.rs::run_from_workspace`/workspace-save (`:906-950`).
- **Agent-activity typed stream (ITEM-5):** mirror how `sandbox_progress.rs` maps a subprocess event
  stream into structured progress entries; append via the existing `step_logs_json` channel used by
  `log_io.rs`. Emit through `progress_sse.rs` like every other track.
- **Builder store (ITEM-6):** `defineLocalStore` per the store-kit model (per-instance, like the
  split-chat pane store) — NOT a global singleton (a builder is an editing session).
- **Builder forms (ITEM-7/8/9):** mirror the settings-form idiom — `Field`/`FieldGroup`/`FieldSet`
  (kit), `SettingsPageContainer`/`Card` layout; the closest rich-form precedent is the
  `llm-provider` model-settings form (typed fields off a schema) and `WorkflowRunDialog`'s IR-driven
  inputs form. The friendly agent form draws its progressive-disclosure + domain-language pattern from
  the handoff's design-tournament winners (editorial rows / numbered checklist) and
  `literature/LiteratureToolResultCard.tsx` (claim-or-delegate friendly card).
- **Run timeline (ITEM-11/12/13):** mirror `WorkflowRunProgressView.tsx`'s existing per-step row +
  `TrackWidget`, and the chat friendly-card renderer pattern (`chatExtensionRegistry.renderContent`);
  gate inline reuses `WorkflowElicitForm.tsx`.
- **Gallery (ITEM-16):** mirror the existing workflow gallery entries + the showcase-modular-seed
  "module owns its gallery.tsx + completeness gate" pattern.

---

## UI-surface plan checklist / JTBD (mandatory — the builder, the agent form, the run timeline)

### JTBD — what a real (non-technical, life-scientist) human wants to DO
- **Build a task without YAML.** "I want the assistant to read my uploaded papers and draft a summary,
  and I want to say that in plain words, pick what it's allowed to use, press save, and run it."
- **Understand what a step will do before running.** A plain-English read-back, not a config dump.
- **Watch the assistant work and trust it.** During a run: see "Searching the literature… Reading 3
  papers… Drafting a summary" — a followable, evolving result stream, not one collapsing log line or a
  bare `timestamp — status`. Be able to open any activity for the underlying detail (sources, tool
  args, output) and to answer/approve when the assistant pauses at a gate.
- **Edit and re-run.** Reopen a saved workflow, change the instructions or the effort, save, re-run.

### Per-surface reconciliation
- **Builder page (ITEM-7)** — *Precedent:* the settings-page container + card idiom; the step list
  mirrors a reorderable settings list (drag like the web-search provider-chain editor). *Scale/cardinality:*
  steps are small-N (typical <20); a workflow's inputs small-N; the tool/server picker lists the user's
  **accessible** MCP servers (bounded, already fetched elsewhere) — no unbounded fetch. *Responsive:* at
  390px the step list stacks above the config panel (master→detail becomes stacked, mirroring the chat
  drawer); gallery includes the 390px state. *Populated render:* gallery seeds a 4-step workflow incl. an
  agent step for the design-critic pass. *Progress:* Save shows validating→saving→saved; validation errors
  itemized inline, not a silent boolean. *Input economy:* every step field is a typed control off the
  StepConfig schema (NEVER a raw-JSON textarea — last-resort only for an unknown `Tool` arguments blob,
  and even then a key/value editor first); references to inputs/prior-outputs are a **picker** (ITEM-10),
  never hand-typed `{{ steps.x.output }}`; output format is a segmented control; effort is a slider.
- **Agent friendly form (ITEM-9)** — *Precedent:* the friendly card + progressive disclosure from the
  handoff. Speaks the DOMAIN (never "run_js"/"MCP"/"max_steps" on the calm surface — "how thorough",
  "what it can use"). Show-then-act read-back. Advanced knobs (system directive, exact step cap) behind a
  disclosure. *Input economy:* capabilities are a labelled multi-select (not server-id text); effort is a
  dial with Quick/Balanced/Thorough stops mapping to `max_steps`.
- **Run timeline (ITEM-12)** — *Precedent:* editorial-rows design-tournament winner + existing
  `WorkflowRunProgressView` step row. *Scale:* an agent run can emit many activities — the timeline
  virtualizes/caps the rendered window and keeps a "showing latest" affordance (no unbounded DOM). *JTBD:*
  each row is ONE domain line + status pill + "Show details" (sources / tool args / output). Gate/approval
  renders inline (reuse `WorkflowElicitForm`). *Responsive:* rows reflow at 390px (status pill wraps under
  the title, not off-screen). *Progress:* running rows show a live indeterminate/step indicator; the
  overall step shows tokens/elapsed (already present). *Populated render:* gallery seeds running / gate-open
  / completed states for the critic pass.
- **Entity-lifecycle (all surfaces):** a workflow deleted while the builder is open, or a run
  cancelled/deleted while the timeline is open, must be handled from BOTH the local action AND the
  `sync:`/SSE path (covered in Phase 5's entity-lifecycle walk + tests).
- **Platform affordances:** the builder is in-app chrome on both web + desktop (no platform-native
  equivalent); no `__TAURI__`-gated affordance needed here.

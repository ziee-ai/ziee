# PLAN_AUDIT — workflow-kind-agent (plan audited against the codebase)

Two read-only codebase-verification sweeps (backend + frontend) checked every plan assumption with
file:line evidence. No `BLOCKED` verdicts. The material adjustments below are already folded back into
PLAN.md (ITEM-3/5/9/13/15 + Files/Patterns). Evidence anchors are cited inline.

## Breakage risk

- **All backend changes are additive.** The 4 new endpoints (ITEM-1..4) add routes + handlers; no
  existing handler signature changes. `import`'s shared core `install_workflow_from_bytes`
  (`handlers/dev.rs:192-343`) is reused, not modified — the new create/update handlers call it (or its
  `pack_workspace_dir`→install tail) after serializing the posted def to a `workflow.yaml` dir.
- **ITEM-5 is the one edit to an agent-core-authored file** (`agent_dispatch.rs` `WorkflowEventSink`,
  `:343-398`) plus a NEW `ProgressKind::AgentActivity` variant in `events.rs`. Adding an enum variant
  to `ProgressKind` (`#[serde(tag="type")]`, `events.rs:223-249`) is backward-compatible on the wire
  (new tag value); every existing `match` on `ProgressKind` in the codebase must gain the new arm —
  the audit will grep these at Phase 6 (a non-exhaustive match is a compile error, so the compiler
  enforces completeness). Risk: LOW, compiler-caught.
- **ITEM-3 in-place definition update** needs a NEW `repository::update_definition` (audit Q5:
  `repository::update` at `:987` is metadata-only; the import path does delete+insert → new id, which
  a re-save must NOT do). Writing a focused id-preserving UPDATE of
  `compiled_ir_json/extracted_path/bundle_sha256/bundle_size_bytes/file_count` is low-risk (mirrors the
  column set `insert` already writes, `:133-187`). Must also delete the OLD extracted bundle dir on
  overwrite (resource-cleanup rule) — noted for Phase 5.
- **FE run-store** (ITEM-13): the new `agentActivity[]` field + SSE handler is additive to
  `WorkflowRun.store.ts`; the existing `tracks` overwrite path (`:194-207`) is untouched (non-agent
  tracks still use it). No regression to other step kinds' progress.
- **Build prerequisite (NEW breakage risk — not in the original plan):** the FE design system is the
  **`@ziee/kit` submodule** (`sdk/packages/kit`), which is **uninitialized in this worktree**
  (audit Q4). `tsc`/build/`gate:ui` will fail until `git submodule update --init sdk` (+ `npm install`
  workspace link) is run. → added to Phase-2 preflight follow-up; must be green before Phase 5.

## Pattern conformance

- **Create/update from a def (ITEM-1/2/3):** mirror `workspace_save` (`handlers/dev.rs:928-950`) —
  serialize `WorkflowDef` → `workflow.yaml` in a dir → `hub::bundle::pack_workspace_dir` → the shared
  `install_workflow_from_bytes`. This is the exact precedent (audit Q2). ✔
- **Validate-def (ITEM-4):** `validate_collecting`/`validate_for_install` (`validate.rs:435/463`) and
  `estimate_static`/`dry_run` (`cost.rs:74/94`) ALL already take `&WorkflowDef` (not YAML/IR), so the
  new handler reuses them directly with a throwaway `temp_dir()` `bundle_root` (matching the existing
  YAML `/validate` at `dev.rs:95-96`). Zero validator refactor. ✔
- **FE forms (ITEM-7/8/9):** corrected to `@ziee/kit` + `FormField` (no `src/components/ui`, no
  `Field*` family — audit Q4). Precedents `WorkflowRunDialog.tsx` (static typed fields) +
  `WorkflowElicitForm.tsx` (`buildElicitZodSchema` + `renderField` dispatcher) — both in-module. ✔
- **MCP capability picker (ITEM-9):** `Stores.McpServer.servers` (`McpServer[]`
  id/name/display_name/enabled/is_system, `mcp/stores/McpServer.store.ts:32`, loaded by
  `loadMcpServers()`→`listAccessible`). Cross-module read via the framework `Stores.McpServer` proxy
  IS the sanctioned public surface (declaration-merged, `mcp/types.ts:16`) — the plan's "avoid raw
  cross-module Stores.X" caveat does NOT apply here (audit Q3). Corrected in ITEM-9. ✔
- **Effort control (ITEM-9):** NO kit `Slider` (audit Q4) → kit `Segmented` with discrete
  Quick/Balanced/Thorough stops → `max_steps`. Corrected. ✔
- **Run timeline / activity stream (ITEM-5/11/12/13):** the `tracks` map is overwrite-by-id
  (`WorkflowRun.store.ts:194-207`) and `ProgressKind` is line/bar/status-only (`events.rs:223-249`) —
  neither carries structured history (audit Q1+backend Q4/Q6). So this needs a NEW ProgressKind
  variant + NEW durable `step_logs_json` append writer + NEW FE store field/SSE handler/render branch,
  NOT a reuse of `tracks`. Corrected across ITEM-5/13. ✔ Gallery: append to the auto-discovered
  `gallery: ModuleGallery` (`gallery.tsx`) `overlays[]`/`seeded[]`; the `WorkflowRunProgressView`
  seeded entries (`:337-418`) are the direct template (audit Q6). ✔
- **Desktop (ITEM-15):** corrected — NO `desktop/ui` workflow mirror exists; desktop reuses shared
  `ui/` modules (`desktop-loader.ts` globs only desktop-specific modules, audit Q7). New FE files are
  authored once. ✔

## Migration collisions

- **NONE.** This feature adds no migration (BASE.md): editable source of truth is the on-disk
  `workflow.yaml` bundle (`workflows.extracted_path`); ITEM-3 updates the existing `compiled_ir_json`
  column in place; ITEM-5 reuses the existing `workflow_runs.step_logs_json` jsonb column (default
  `'{}'`, un-cleared) — confirmed it already holds structured objects (`persist_step_logs`,
  `repository.rs:636-662`), only a new append writer is needed, no schema change. No new permission →
  no grant migration. Highest module migration remains `202607170100_workflow_agent_step`.

## OpenAPI regen

- **REQUIRED, both binaries.** New endpoints (`GET/PUT /workflows/{id}/definition`, `POST /workflows`,
  `POST /workflows/validate-def`) + newly-exposed types: `WorkflowDef`/`StepDef`/`StepConfig`/
  `OutputFormat`/`InputDef` (all already `#[derive(JsonSchema)]`, `validate.rs:55-148`) + the new
  `ProgressKind::AgentActivity` variant. `just openapi-regen` writes BOTH `ui/` and `desktop/ui/`
  api-clients (memory: openapi-regen does both); the `emit_ts` golden-parity test must stay green
  (ITEM-14). Confirmed `WorkflowDef`/`StepDef`/`StepConfig` are ABSENT from `api-client/types.ts`
  today (audit Q8) — so regen is what introduces them.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `GET /workflows/{id}/definition` parses `workflow.yaml` from
  `extracted_path`; mirrors `get_user_workflow` owner-scope. Exposes the JsonSchema types via regen.
- **ITEM-2** — verdict: PASS — `POST /workflows` reuses `pack_workspace_dir`+`install_workflow_from_bytes`
  (audit Q2). The deliberately-absent create endpoint is now justified (builder needs a non-tarball
  source). Gate `WorkflowsInstall`.
- **ITEM-3** — verdict: CONCERN (resolved in plan) — needs a NEW id-preserving
  `repository::update_definition` (metadata `update` insufficient; delete+insert would change the id).
  Must also delete the superseded bundle dir. Folded into PLAN.md + Files.
- **ITEM-4** — verdict: PASS — `validate`/`cost` already take `&WorkflowDef` (audit Q3); direct reuse
  with a temp `bundle_root`. Zero refactor.
- **ITEM-5** — verdict: CONCERN (resolved in plan) — no existing append channel + `ProgressKind`
  doesn't fit structured activity (backend Q4/Q6). Now specified as: new `ProgressKind::AgentActivity`
  variant + new `step_logs_json` seq-keyed append writer (no migration) + FE parsing. Compiler enforces
  the exhaustive-match fix-up across `ProgressKind` consumers.
- **ITEM-6** — verdict: PASS — FE `WorkflowDef`/`StepConfig` types arrive via regen (ITEM-14);
  `WorkflowBuilder.store.ts` via `defineLocalStore` (per-instance editing session, store-kit model).
- **ITEM-7** — verdict: CONCERN (resolved in plan) — routes are **Settings-scoped**
  (`/settings/workflows/...`, `SettingsLayoutDef`), not top-level sidebar (audit Q2); entry affordances
  = the WorkflowsList Import button + the WorkflowDetailDrawer action cluster (no per-row actions exist).
- **ITEM-8** — verdict: PASS — per-kind forms mirror `WorkflowElicitForm`'s schema-driven
  `renderField` dispatcher + `WorkflowRunDialog`'s typed-field layout (audit Q5).
- **ITEM-9** — verdict: CONCERN (resolved in plan) — `@ziee/kit`+`FormField` (no `Field*` family),
  `Segmented` effort control (no `Slider`), `Stores.McpServer.servers` picker (audit Q3/Q4). Corrected.
- **ITEM-10** — verdict: PASS — ref-insert menu over compiled-IR inferred types; a linear picker, not a
  graph editor. (Descope candidate per the confirmed discipline if Phase 5 gets heavy.)
- **ITEM-11** — verdict: PASS — activity-descriptor registry is a pure FE map + fallback; mirrors the
  handoff F1. No codebase dependency.
- **ITEM-12** — verdict: PASS — timeline renderer mounts in `WorkflowRunProgressView` for agent steps,
  reads the new `agentActivity[]`; gate/approval reuses `WorkflowElicitForm`; kit Card/Badge/Accordion/
  Button + app-level `ToolStatusIcon` (chat module, reusable — audit Q4).
- **ITEM-13** — verdict: CONCERN (resolved in plan) — `tracks` can't carry history (audit Q1); now a
  new `agentActivity[]` store field + SSE handler + snapshot array-merge. Corrected.
- **ITEM-14** — verdict: PASS — `just openapi-regen` both workspaces; golden parity test guards it.
- **ITEM-15** — verdict: CONCERN (resolved in plan) — NO desktop workflow mirror (audit Q7); item
  reduces to build-verify + desktop api-client regen + a coverage-classifier string. Big simplification.
- **ITEM-16** — verdict: PASS — gallery surfaces append to the auto-discovered `gallery` object
  (`overlays[]`/`seeded[]` + `setup`→`holdPatch(setState)`); `WorkflowRunProgressView` seeded entries
  are the template; `surface:` strings drive `gate:ui` coverage (audit Q6).

## Follow-up before Phase 5 (not gating Phase 2)
- Run `git submodule update --init sdk` (+ `npm install`) so `@ziee/kit` resolves — else FE build/tsc/
  gate:ui fail. Re-run `preflight.sh` after.
- Grep all `match … ProgressKind` sites so ITEM-5's new variant is handled everywhere (compiler-caught,
  but enumerate at implement time).

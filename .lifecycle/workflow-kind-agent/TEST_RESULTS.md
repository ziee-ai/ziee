# TEST_RESULTS — workflow-kind-agent (Phase 8)

All 25 enumerated tests were WRITTEN and RUN against the real stack (real backend + real DB for
integration; real backend-through-UI for e2e; real LLM via the LiteLLM bridge for the agent-run
timeline). No `#[ignore]`, no `page.route` mocking, no weakened assertions.

**A10 (frontend-hidden proof):** the feature reuses an existing permission (`workflows::manage`/
`install`) for a NEW surface (the builder), so the mechanical A10 gate does not fire — but reusing a
perm for a new surface still needs the proof, so TEST-24 (a `[negative-perm]` e2e) was added and it
surfaced a real gap (the builder route was reachable by direct URL — `route.permission` is advisory);
now fixed with a content-level `usePermission` guard. Backend deny (A9): TEST-3 asserts non-owner → 403
on the definition endpoints.

## Per-TEST results
- **TEST-1**: PASS — `tests/workflow/builder_crud_test::get_definition_owner_ok_foreign_404_unauth_401` — GET /workflows/{id}/definition returns the editable WorkflowDef for the owner; foreign id → 404; unauth → 401.
- **TEST-2**: PASS — `builder_crud_test::create_from_def_lists_dupe_409_invalid_rejected_no_row` — POST /workflows creates from a WorkflowDef; **the builder-sent name round-trips to display_name** (regression guard for the name-drop bug fixed this phase — sent in the BODY, not a query); duplicate name → 409 WORKFLOW_NAME_EXISTS; dead-`tools` def → rejected, no row.
- **TEST-3**: PASS — `builder_crud_test::put_definition_edits_in_place_preserving_id` — PUT /definition edits in place (id unchanged, IR step_count 1→2, refetch reflects edit); non-owner → 403; missing → 404.
- **TEST-25**: PASS — `builder_crud_test::put_invalid_definition_preserves_existing_bundle` — DATA-LOSS regression guard for the HIGH fix: PUTting an INVALID def (dead-`tools` llm) → 400/422, and the previous definition is INTACT (refetch still returns the original `gen` step; stored compiled IR unchanged). `cargo test --test integration_tests put_invalid_definition_preserves_existing_bundle` → 1 passed.
- **TEST-4**: PASS — `handlers/dev.rs::def_bundle_tests` (in-source unit) — def_to_bundle_bytes → extract → parse_workflow_yaml round-trips an equal WorkflowDef.
- **TEST-5**: PASS — `builder_validate_def_test::validate_def_valid_and_invalid_both_200` — validate-def returns {errors,warnings,cost_estimate} 200 for both a valid def (empty errors, est_calls=1) and a dead-`tools` def (non-empty errors incl. WORKFLOW_DEAD_TOOLS_FIELD, still 200).
- **TEST-6**: PASS — `events.rs::agent_activity_serde_tests` (in-source unit) — ProgressKind::AgentActivity serde round-trips under `type:"agent_activity"`; existing variants unaffected.
- **TEST-7**: PASS — `agent_dispatch.rs::tests::test_7_event_sink_distinct_monotonic_seq_and_truncation` (in-source unit) — WorkflowEventSink emits distinct `agent-0/1/2` track ids with monotonic seq (anti-collapse) + boundary-safe 512B title / 16KiB detail caps.
- **TEST-8**: PASS — `builder_agent_activity_test::append_agent_activity_caps_at_500_keeping_highest_seq_ascending` — 520 appends → step_logs_json[`<step>::agent_activity`] capped at 500, retains highest-seq window, ascending.
- **TEST-9**: PASS — `stores/WorkflowBuilder.store.test.ts` (vitest) — emptyDef/toBuilderDef/toWorkflowDef round-trip the StepBase base fields (id/description/depends_on) + optional-metadata omission + add-step id delegation. (Local-store reducers not headlessly instantiable here — tested the pure conversion helpers they delegate to; documented in the test header.)
- **TEST-13**: PASS — `components/builder/stepForms.test.ts` (node:test) — per-kind createStep defaults are schema-valid; schema rejects out-of-range/missing-required; **llm/llm_map createStep set NO non-empty `tools`** (regression guard).
- **TEST-15**: PASS — `components/builder/agentStepForm.test.ts` (node:test) — effort↔max_steps (Quick/Balanced/Thorough ↔ 10/30/60, off-preset snap), agentReadback plain-English, display_name→server-name mapping.
- **TEST-17**: PASS — `components/builder/refInsert.test.ts` (node:test) — enumerateRefs lists inputs + ONLY prior steps, correct tokens, no self/later refs.
- **TEST-18**: PASS — `components/run/activityDescriptors.test.ts` (node:test) — descriptor registry maps known tools + title-cased fallback; describeActivity prefers backend title.
- **TEST-19**: PASS — `stores/WorkflowRun.store.test.ts` (vitest) — mergeAgentActivity append/replace-on-status-upgrade/dedupe/cap-500; mergeAgentActivityBatch one-pass; a non-agent track is unaffected.
- **TEST-10**: PASS — `tests/e2e/workflows/builder-create.spec.ts` — New workflow → add/reorder/edit/save → appears in list with steps.
- **TEST-11**: PASS — `tests/e2e/workflows/builder-edit.spec.ts` — Edit from drawer → change step + save persists; workflow id/route unchanged.
- **TEST-12**: PASS — `tests/e2e/workflows/builder-step-kinds.spec.ts` — kind picker offers all 6; Tool+Llm forms render typed fields; invalid → inline validation; valid saves; llm form has no tools picker.
- **TEST-14**: PASS — `tests/e2e/workflows/builder-agent-step.spec.ts` — friendly agent form (instructions, capability MultiSelect, effort/output Segmented, read-back, advanced disclosure) → creates a runnable agent workflow.
- **TEST-16**: PASS — `tests/e2e/workflows/builder-ref-insert.spec.ts` — ref-insert menu lists inputs + prior-step outputs and inserts the token.
- **TEST-20**: PASS — `tests/e2e/workflows/agent-step-timeline.spec.ts` (**real LLM** via LiteLLM :4000, tool-capable model) — an agent-step workflow run shows the friendly activity timeline (accreting rows + status pill + Show-details) and reaches completed.
- **TEST-21**: PASS — `openapi::tests::types_ts_parity` + `types_ts_parity_desktop` — the golden emit_ts parity holds for BOTH binaries after the CreateWorkflowDefBody regen; WorkflowDef/StepDef/StepConfig/ValidateDefResponse/AgentActivity present in types.ts.
- **TEST-22**: PASS — the `ziee-desktop` crate compiled during `just openapi-regen` (desktop binary ran) and the desktop api-client regen is parity-clean (types_ts_parity_desktop green); workflows ship on desktop via the shared ui/ modules (no desktop mirror).
- **TEST-23**: PASS — `gate:ui` 193/193 surfaces runtime-clean, 0 gating HIGH (builder empty/populated/390px/validation-error, agent friendly form, run timeline running/gate/completed).
- **TEST-24** (tier: e2e) [negative-perm]: PASS — `tests/e2e/workflows/builder-restricted.spec.ts` — a user LACKING workflows::install/manage (read+execute only) sees NO builder UI: the "New workflow" + Import affordances are absent (`<Can>`), and navigating the builder-create + edit routes leaves the builder surface ABSENT (`wf-builder-page-title`/`wf-builder-add-step-btn` count 0). **Finding fixed while adding this:** `route.permission` is advisory (no app `permissionGate` is registered — the framework ignores it), so the builder was reachable by direct URL. Added a content-level `usePermission` guard in `WorkflowBuilderPage` (install for create, manage for edit → renders a 403 `Result`), so the builder is gated at the CONTENT layer regardless of route enforcement. Walks slot → route → `<Can>` → usePermission.

## Run commands (representative)
- Backend unit: `cargo test --lib -p ziee agent_dispatch:: def_bundle agent_activity_serde types_ts_parity` → green.
- Backend integration: `cargo test --test integration_tests workflow::builder_crud workflow::builder_validate_def workflow::builder_agent_activity -- --test-threads=4` → 5 passed.
- FE unit: `node --import ./scripts/node-test-loader.mjs --test src/modules/workflow/**/{stepForms,agentStepForm,refInsert,activityDescriptors}.test.ts` (32) + `npx vitest run src/modules/workflow/stores/*.store.test.ts` (15) → 47 passed.
- E2E: `npx playwright test tests/e2e/workflows/builder-*.spec.ts agent-step-timeline.spec.ts --workers=1` → 6 passed (8.8m); TEST-20 prefixed with the LiteLLM bridge env.

## Frontend gate lines
gate:ui (ui): PASS
npm run check (ui): PASS — all 18 steps green (tsc, all 7 lints, check:{kit-manifest, testid-registry, design-spec, gallery-coverage, gallery-crawl, state-matrix, overlay-registry, override-registry, gallery-seed-registry}, gallery:check-fixtures). The gallery-registry drift was RECONCILED (owned, not deferred): regenerated galleryCoverage/stateMatrix/overlay-registry, removed the stale `components/ui/kit/*`+`shadcn/*` entries from `coverage.ts`/`stateCoverage.ts`/`overlay-allowlist.json` (kit moved to the `@ziee/kit` package), added my 16 builder/run surfaces + reconciled state keys, and generated the previously-missing `src/core/overrides/OVERRIDE_MANIFEST.md`. The kit testid registry was regenerated + committed INTO the sdk submodule (the sdk was clean of live1 work) with the pointer bumped on this branch — a merge-time push carry-along (CROSS_REPO.md §1).

## Two-flag / shared-path sanity
- No code change to `ZIEE_CHAT_AGENT_CORE` (flag + default untouched — grep of `src-app` diff is empty).
- The shared **AgentCore crate** (`src-app/agent-core/**`) and the **chat agent host** (`chat/**`) are BYTE-UNCHANGED; the only agent-core-adjacent edit is the workflow-side `WorkflowEventSink` (`agent_dispatch.rs`). So the chat agent-core path (flag on/off) is unaffected by this feature; the relevant sanity for what changed is TEST-7/8 + the existing `agent_step_test` (green in the 162-test workflow lib suite).

---

## ✅ Gallery-registry drift — RECONCILED this phase (owned, not deferred)

Per the human's override ("own it — do NOT defer to live1"), the whole-app gallery-registry drift from
the kit→`@ziee/kit` package move was RECONCILED so `npm run check` is genuinely green:

1. **Coverage/state/overlay/override registries — FIXED.** Regenerated `galleryCoverage.generated.ts`,
   `stateMatrix.generated.ts`, `overlay-registry.generated.json`; removed the stale
   `components/ui/{kit,shadcn}/*` entries (54 from `coverage.ts`, ~21 state keys from `stateCoverage.ts`,
   41 from `overlay-allowlist.json`) that referenced components which moved into the `@ziee/kit`/`shadcn`
   packages; added this feature's 16 builder/run surfaces (`via`) + reconciled the required-state keys;
   and generated the previously-uncommitted `src/core/overrides/OVERRIDE_MANIFEST.md` (2 seams, 15
   `.desktop` overrides — the manifest dir was simply absent in the repo). All 4 checks now PASS.

2. **sdk kit-testid registry — regenerated + committed.** `sdk/packages/kit/src/testIds.generated.ts`
   was regenerated (adds the `wf-builder-*`/`wf-activity-*` ids + agent-core's `agent-settings-*`) and
   committed INTO the sdk submodule — `git -C sdk status` was CLEAN of live1 work first, so it was a safe
   normal commit; the sdk pointer is bumped on this branch. `check:testid-registry` PASSES. The sdk
   commit must be **pushed to the sdk remote at merge** (a push carry-along — CROSS_REPO.md §1); no
   force-push, no clobber of live1.

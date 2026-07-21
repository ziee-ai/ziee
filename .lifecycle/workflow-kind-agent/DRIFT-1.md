# DRIFT-1 — implementation vs plan (workflow-kind-agent)

Deviations found while implementing Phase 5, each reconciled against PLAN.md / DECISIONS.md.

- **DRIFT-1.1** — verdict: impl-wins — Agent `servers` selection maps to server **NAME**, not id. The
  plan/FE spec said "id list", but the backend resolves an agent/tool step's server allow-list by NAME
  at run time (`resolve_tool_server`; the `StepConfig::Agent.servers` is a name allow-list). Sending ids
  would fail resolution. The capability `MultiSelect` therefore uses `server.name` as the value with
  `display_name` as the label. Correct; the spec was wrong.

- **DRIFT-1.2** — verdict: impl-wins — Durable agent activity is stored under the jsonb key
  `step_logs_json["<step_id>::agent_activity"]` (an ordered array), NOT the bare `<step_id>` key. The
  bare key is already owned by `persist_step_logs` (log-kind → {path,size,preview,body}); reusing it
  would clobber captured stdout/stderr. Same jsonb-set idiom, distinct namespace — a strictly safer
  realization of ITEM-5c. No migration (column unchanged). FE snapshot rehydrate parses the
  `::agent_activity`-suffixed keys accordingly.

- **DRIFT-1.3** — verdict: resolved — The generated TS `StepDef` is lossy: the backend
  `#[serde(flatten)]`s `StepConfig` onto `StepDef`, and the schemars→TS generator emits only the config
  `oneOf`, dropping the base fields (`id`/`description`/`message`/`depends_on`/…). **Runtime wire is
  correct** (serde flatten round-trips the fields); only the generated *type* is incomplete, so the FE
  reconstructs a `StepBase` and uses `BuilderStep = StepBase & StepDef` (assignable back to
  `WorkflowDef.steps`) — payloads round-trip cleanly and tsc is green. Resolved for this feature.
  **Follow-up (logged, non-blocking):** a cleaner fix is to make `StepDef`'s JsonSchema represent the
  flattened base fields (or stop flattening) so the generated type is complete — a schema-gen
  improvement, out of this feature's scope; noted for a future pass. Does not affect correctness here.

- **DRIFT-1.4** — verdict: none — `#[debug_handler]` omitted on the new handlers. Not a drift from the
  codebase: the entire workflow module omits it (`get_user_workflow`/`import_workflow`/…), and the
  OpenAPI regen + both `types_ts_parity` tests pass, so aide is satisfied. Matches "mirror existing
  patterns" over the general guideline.

- **DRIFT-1.5** — verdict: none — ITEM-3 (edit definition) gates `WorkflowsManage`, ITEM-2 (create)
  gates `WorkflowsInstall`. This matches DECISIONS DEC-1 exactly (edit=manage, create=install); the
  brief transiently said "install" for both — DEC-1 governs. No drift.

**Unresolved drifts:** 0

# DRIFT-3 — Round 3 (FB-9 precedent audit): implementation vs plan

Reconciling the shipped Round-3 code against PLAN Round-3 items.

- **DRIFT-3.1** — verdict: resolved — ITEM-56 planned "wrap the Schedule block in a
  labelled FormField". Implemented as a labelled `Field`+`FieldTitle` wrapper instead:
  `FormField` clones its child and injects `value`/`onChange`, but `ScheduleBuilder` types
  those props as REQUIRED, so `<ScheduleBuilder/>` inside `FormField` fails tsc (missing
  props). A labelled `Field` wrapper (with the existing `value`/`onChange` binding + zod
  validation blocking submit + the footer onInvalid surfacing the message) meets the item's
  INTENT (a labelled, Field-idiom, resolver-validated schedule control). Segmented + the
  static text/entity fields DO use `FormField`. No behavior gap.
- **DRIFT-3.2** — verdict: resolved — ITEM-58 label rows converted `<label>` → `Field`+
  `FieldTitle`; to preserve the accessible name the native `<label>` provided, each control
  gained an explicit `aria-label` (Run at / Time / Day of month). Matches the a11y contract.
- **DRIFT-3.3** — verdict: resolved — generated gallery manifests regenerated (coverage,
  state-matrix, testid-registry, overlay-registry) + `ScheduledTaskCard` added to
  coverage.ts/overlay-allowlist and its `:empty/:error/:open` run-states allow-listed in
  stateCoverage.ts (run-data-dependent, e2e-verified). `ScheduledTasksPage:empty` mapping
  removed (the reworked page no longer emits that required-state key).

**Unresolved drifts:** 0

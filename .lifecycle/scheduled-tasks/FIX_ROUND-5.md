# FIX_ROUND-5 — Round 3 (FB-9) blind-audit remediation

Merged the 3 blind-audit angles (correctness · UI-precedent · contract/tests/scale).
14 findings triaged; 13 fixed, 1 rejected with rationale.

## Fixed
- **correctness (medium)** ScheduledTaskFormDrawer — moved the raw `inputs_json`
  JSON-validity check OUT of the zod schema (which fired even in typed-input mode,
  blocking Save on a stale value for a non-rendered Textarea) into `onSubmit`,
  gated on `!hasDeclaredInputs`.
- **error-handling (medium)** ScheduledTaskCard — setEnabled / runNow / delete now
  try/catch → `message.error` (store actions don't set `error`, so failures were
  silently swallowed). fork (continueRun) + series (continueSeries) likewise
  wrapped (were unhandled rejections with silent no-nav).
- **design-in-context (medium)** ScheduledTaskCard — moved the kind Tag + status
  Badge OUT of the truncating CardTitle into a `Flex flex-wrap` chips row in the
  card body (mirrors KnowledgeBaseCard/ProjectCard) so they never clip at narrow
  widths.
- **state-management (low)** ScheduledTasksPage — the mutation-error effect now
  calls `clearError()` after toasting (was re-toasting a stale error on any
  tasks.length change).
- **correctness (low)** ScheduledTaskFormDrawer — added an inline `FieldError`
  under the Schedule Field so a schedule validation error surfaces on the
  Enter-to-submit path (not only the footer Save's onInvalid).
- **affordance-parity (low)** ScheduledTaskCard — delete button now shows the
  per-card `loading={deleting}` spinner (in-flight feedback, like the twins).
- **precedent-fidelity (low)** ScheduledTasksPage — list gap-2 → gap-3; cold-load
  Spin `size="lg"` → default (match KB/projects).
- **test-reality (medium+low)** precedent-layout.spec — TEST-60 now reads the
  action wrapper's COMPUTED opacity (0 at rest → 1 on hover) instead of a
  toBeVisible that ignores opacity; TEST-63 likewise asserts opacity→1 at 390px;
  TEST-61 tightened to `weight === 400`.

## Rejected (with rationale)
- **scale-performance (low)** Load-More over a full `.list()` fetch — ACCEPTED by
  precedent: KnowledgeBasesListPage/ProjectsListPage/chat all fetch-all + client-
  slice; the task set is bounded by the admin `max_active_tasks_per_user` cap. Not
  a defect; mirroring the sibling was the explicit instruction (DEC-24).

**New confirmed findings:** 0

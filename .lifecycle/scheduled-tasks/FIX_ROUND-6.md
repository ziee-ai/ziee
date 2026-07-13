# FIX_ROUND-6 — Round 3 (FB-9) second remediation + convergence

The blind round after FIX_ROUND-5 surfaced 2 findings (both from FIX_ROUND-5's own
changes); fixed here, then a fresh full blind round yielded 0.

## Fixed
- **state-management (medium)** ScheduledTaskFormDrawer — `setValue('schedule', next,
  { shouldValidate: form.formState.isSubmitted })` so the inline schedule FieldError
  (added in FIX-5) clears once the user corrects the schedule, without validating
  before the first submit. (ScheduleBuilder isn't an RHF-registered field, so the
  default onChange re-validation never fired for it.)
- **a11y (low)** ScheduledTaskFormDrawer — `<Segmented aria-label="Type">`: the kit
  Segmented forwards aria-label but not the aria-labelledby FormField injects, so the
  target-kind group had no accessible name; the explicit aria-label supplies it.

## Convergence
A fresh full blind round (correctness · state-management · error-handling · a11y ·
precedent-fidelity · test-reality · Rules-of-Hooks) over the entire current diff
returned **NO NEW FINDINGS**, cross-checking the shouldValidate wiring, the Segmented
aria path, the zod/JSON-mode matrix, the card error handlers + chips move, and the
e2e computed-opacity assertions.

**New confirmed findings:** 0

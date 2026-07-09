# FIX_ROUND-6 — js-tool-scripting (verification of FIX_ROUND-5)

A blind verification pass over the three FIX_ROUND-5 fixes. Two were confirmed
correct; one was still wrong:

- **F2 (e2e reload race)** — CONFIRMED FIXED: the `waitForResponse` predicate
  matches the PUT to `/api/js-tool/settings`, is registered before `save.click()`,
  resolves before `reload()`, and `putResp.ok()` asserts success.
- **F3 (a11y units in labels)** — CONFIRMED FIXED: all four labels match their
  suffix + conversions (MiB↔1024², KiB↔1024, seconds↔s); every field keeps a label.
- **F1' (order collision) — NEW confirmed finding** — FIX_ROUND-5 moved the
  `settingsAdminPages` order to 28, but 28 ALSO collides (workflow "System
  Workflows"=28). RE-FIXED to `order:23`, deterministically verified free against
  the full set of settingsAdminPages orders (10,11,16,17,20,21,22,25,26,27,28,29,
  30,40,51,52,53,60,61,65,100 — 23 is absent).

**New confirmed findings:** 1

# FIX_ROUND-5 — js-tool-scripting (admin-configurable limits increment audit)

A fresh 4-angle blind round over the increment's source + tests (no reasoning
handed to the reviewers):

- **correctness-and-security** (migration/settings/cache/repository/sync/store/section/module)
- **patterns-conformance** (whole increment vs the code_sandbox reference)
- **tests-quality / state-management / a11y** (integration + e2e tests, store/section)
- **error-handling / api-contract** (gap-fill over all 15 hunks)

Two angles (correctness-and-security, error-handling+api-contract) returned **0
findings** — the increment is a faithful, contract-consistent mirror of
code_sandbox with exact bound/default parity and safe fallbacks.

## Confirmed findings (all FIXED)

- **F1 (patterns-conformance, low)** — `module.tsx` `settingsAdminPages` `order:27`
  collided with Web Search + System Skills (both 27). FIXED: `order:28` (free slot
  after Code Sandbox's 26).
- **F2 (tests-quality, medium)** — the persist e2e reloaded immediately after
  `save.click()` with no wait for the PUT → the reload could cancel the in-flight
  save (latent race). FIXED: `await page.waitForResponse(<PUT /js-tool/settings>)`
  and assert `ok()` before reloading.
- **F3 (a11y, low)** — the byte/time units lived only in the decorative InputNumber
  `suffix`, outside each field's accessible name. FIXED: units moved into the
  `FormField` labels ("Memory limit (MiB)", "Stack size (KiB)", "Wall-clock timeout
  (seconds)", "Approval timeout (seconds)").

**New confirmed findings:** 3

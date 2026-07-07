# FIX_ROUND-1 — desktop-ui-guardrail-parity

Merged the LEDGER, fixed every open confirmed finding, then re-ran a full
multi-angle pass over the diff.

## Confirmed findings from Phase 6 + disposition

- **F1** (correctness, `vite-plugin-testid-unique.js:44`) — the desktop
  testid-unique plugin diverged from web (missing the `DefectRepro.tsx`
  `TESTID_EXEMPT`) and aborted the gallery build. **Already remediated in this
  diff** (ported the exemption const + `!TESTID_EXEMPT.test(full)` guard). Verified:
  the desktop gallery dev server boots and the full audit ran against it.

- **runLint missing-script robustness** (error-handling,
  `detector-acceptance.mjs:64`) — `runLint` treated any non-zero exit as the
  detector "firing", so a genuinely MISSING detector script would be miscounted
  as a pass. **Fixed:** added an `fs.existsSync(scriptPath)` guard that returns a
  distinct `MISSING detector script` failure (fired:false, exit:-1) so a dropped
  detector fails the acceptance run loudly. Verified: `detector-acceptance.mjs`
  still PASSES on the present detectors, and `detector-acceptance.test.ts` green.

## Re-audit pass (all 10 angles, over the updated diff)

- **correctness** — the runLint guard is fail-closed; the geometry byte-identity
  guard fails loudly on drift/missing web source; overlay-registry.generated.json
  is in-sync. No new issue.
- **error-handling** — missing-detector path now explicit; script spawns capture
  stdout+stderr; no unhandled rejection (detector-acceptance is sync `main()`).
- **security** — fixed-argv spawns, no shell; allowlists have no runtime effect;
  no secret/path injection surface. No new issue.
- **patterns-conformance** — copies remain byte-identical to web source
  (drift-guarded); package.json mirrors web ordering; overlays.tsx shape matches
  web. No new issue.
- **api-contract / perf / concurrency / state-management / a11y / tests-quality** —
  unchanged from Phase 6 review; the fix touched only the missing-script branch of
  one dev script. No new issue.

**New confirmed findings:** 0

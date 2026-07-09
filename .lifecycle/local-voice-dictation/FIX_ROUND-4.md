# FIX_ROUND-4 — audit the post-phase-5 code (tests + gate), fix, re-audit

The phase-8 test-writing added a batch of code (backend integration suite, harness
edits, frontend gallery-gate files) that the phases 1–7 audits never saw. A fresh
blind audit of that code found the backend tests genuinely solid (real
stub-whisper-server spawn, real fail-closed sha, real download pipeline) but
caught a real honesty defect in the gallery deferrals:

- **[HIGH] via-a-non-rendering-parent** — the 5 voice admin cards were `kind:'via'`
  the voice settings page, but that parent is itself `pending` (no gallery cell),
  so the cards got zero real coverage while marked covered. **Fixed:** cards →
  `pending`.
- **[MED×2] fictional e2e references** — the deferral reasons cited "14-voice e2e
  specs" that did not exist on the branch yet. **Fixed:** the e2e specs were
  written + committed (`4da19467a`), so the references became real (then further
  corrected in round 5, below).

## Re-audit (round 4 → blind pass over the new code + the e2e specs)

A fresh blind agent reviewed the committed e2e specs (which round 4 introduced) +
the gallery files. It confirmed the 9 specs are REAL (verified selector-by-selector
against the product source; every negative assertion genuine; no cosmetic passes)
but found the round-4 honesty fix was still INCOMPLETE:

- 6 `stateCoverage.ts` `:error`/`:empty` entries still claimed "covered by
  <14-voice spec>", but no spec drives those failure branches (routeVoice serves
  200s).
- 1 low: `mic-recording-ux.spec.ts` raced the transient "Recording started"
  announcement against a 1 s auto-stop.

**New confirmed findings:** 7

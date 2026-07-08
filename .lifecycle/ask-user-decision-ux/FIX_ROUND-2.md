# FIX_ROUND-2 — fix the round-1 re-audit findings, then re-audit

## Fixed (the 5 round-1 re-audit findings)

- **CONC-HIGH (double-POST) + CONC-MED (submit/decline race):** `handleSubmit` now
  sets `isSubmitting` SYNCHRONOUSLY at the top (before `await form.trigger()`),
  inside a try/finally that re-enables on the validation-fail early return. React
  flushes the disabled/loading state between discrete clicks, so a second click
  (or a Submit-then-Decline) can no longer re-enter and issue a conflicting POST.
- **CORR-LOW (submit type):** Submit button now sets `type="button"` (sibling parity
  + kills the latent onClick-plus-native-submit double-fire).
- **A11Y-LOW + PATT-LOW (the `"other"` collision):** the Other control's DOM
  `id`/`labelId` and checkbox `data-testid` are now derived from `OTHER_SENTINEL`
  instead of the literal `"other"`, so a realistic enum value named `"other"` no
  longer collides.

## Round-2 re-audit (1 fresh blind agent, all angles on the re-fixed diff)

VERIFIED as correct: the concurrency guard is now airtight (no remaining
double-POST / submit-vs-decline race), the server marker-strip is complete (all 3
external ingress sites call `cap_requested_schema`; the only re-stamp runs after
the cap on the internal path), and the sentinel-derived ids / aria-labelledby /
focus-announce carry no a11y regression. It surfaced only LOW items, all triaged
as non-defects:

- **REJECTED (by design):** "`x-ziee-recommended` only honored for enum shapes,
  not titled `oneOf`/`anyOf`." This matches DEC-4: `x-ziee-recommended` is the
  ENUM-shape convention; titled options mark recommended via a per-entry
  `recommended: true`, which `getRichOptions`/`fromEntries` already honors. Both
  shapes support recommended; no defect.
- **REJECTED (accepted risk):** "an enum value literally equal to `OTHER_SENTINEL`
  (`__ziee_other__`) collides with the Other control ids / rewrites via
  `finalizeValues`." `OTHER_SENTINEL` is a RESERVED token by design (unit-asserted
  distinct from realistic values); the realistic `"other"` collision is fixed.
  Hardening every pure helper against a caller supplying the reserved sentinel as
  real data is disproportionate over-engineering.
- **REJECTED (appropriate test layer):** "the wizard component logic (step jump,
  isSubmitting guard, step math) has no UNIT coverage." The agent could not see the
  e2e specs (outside its diff scope): step navigation + Back-preservation are
  covered by TEST-12, the Other-blank validation gating by TEST-18, and the pure
  helpers by the unit suite. RHF-driven component concurrency is correctly exercised
  at the e2e layer, not the unit layer.

No MEDIUM/HIGH findings in this round, and every LOW is a by-design or
accepted-risk non-defect requiring no code change.

**New confirmed findings:** 0

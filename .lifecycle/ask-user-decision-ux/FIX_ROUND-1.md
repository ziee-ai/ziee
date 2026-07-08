# FIX_ROUND-1 — fix the phase-6 ledger, then re-audit

## Fixed (all 14 confirmed phase-6 ledger findings)

- **SEC-HIGH (forged marker):** `cap_requested_schema` now STRIPS `x-ziee-askuser` at every elicitation ingress; only the trusted internal path re-stamps it AFTER the cap (`stamp_ask_user_marker`). External MCP servers can no longer trigger rich mode. + unit test `cap_requested_schema_strips_forged_ask_user_marker`.
- **CORR-HIGH (empty-properties crash):** guarded `total===0` — safe render (message + Decline/Submit, submits `{}`), no destructuring crash.
- **CONC-MED (double-POST):** set `isSubmitting` + re-entry guard on submit/decline (see round-1 re-audit below — this first attempt was INCOMPLETE and was re-fixed).
- **A11Y-MED ×3:** group accessible name (RadioGroup `aria-label` / multi `role=group aria-label` = question title); replaced the terse `aria-label` with `aria-labelledby` to the full option label (description + preview + Recommended now in the accessible name); `aria-live` step indicator + focus-move to the question on step change.
- **CORR-MED (decline catch):** added try/catch parity.
- **CORR-LOW (submit step order):** jump to the globally-first offending step across zod ∪ Other.
- **API-LOW (sentinel collision):** `finalizeValues` only rewrites a field that actually offers Other (`allowsOther`).
- **PATT-LOW:** multi checkboxes now use the kit `Checkbox` (not the shadcn primitive).
- **PERF-LOW:** memoized option/schema builds.
- **TEST-MED ×2:** extracted `finalizeValues`/`otherFieldError`/`isOtherSelected` to `elicitationOptions` + unit tests (single + multi + Other-merge + collision guard); added e2e Other-blank-validation + multi-select roundtrip; corrected the `:error` skip reason.
- **I18N-LOW:** softened the descriptor's "1–4 questions" to guidance.
- **PERF-LOW (parent unused form) — rejected:** hooks-rules require the unconditional `useForm`; cost negligible; extracting the legacy body risks the deliberately-untouched legacy path.

## Round-1 re-audit (2 fresh blind agents on the fixed diff)

The blind re-audit VERIFIED the security marker-strip is complete (all 3 external
ingress sites + persist path + single internal stamp), and that a11y targets /
focus / announce and the new tests are real. It surfaced NEW confirmed findings:

- **CONC-HIGH:** the double-POST re-entry guard was INEFFECTIVE — `setIsSubmitting(true)` still ran AFTER `await form.trigger()`, so the async window let a second click (or Submit-then-Decline) re-enter with `isSubmitting` false and double-POST (a 404 then flips accept→cancelled). → Re-fixed: `setIsSubmitting(true)` now runs synchronously at the top of `handleSubmit`, inside a try/finally that re-enables on validation-fail.
- **CONC-MED:** same root cause enabled a Submit-then-Decline race → fixed by the same synchronous guard (Decline's `loading={isSubmitting}` now disables during submit).
- **CORR-LOW:** Submit button lacked `type="button"` (inert today only because the footer is outside the form) → added for latent-safety + sibling consistency.
- **A11Y-LOW + PATT-LOW (same root cause):** an enum value literally `"other"` collided with the Other-escape's `-other` DOM id and checkbox `data-testid` → derived the Other control's id/label/testid from the reserved `OTHER_SENTINEL` instead, so collision now requires an enum value equal to the sentinel itself.

**New confirmed findings:** 5

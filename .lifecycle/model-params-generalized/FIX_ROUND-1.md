# FIX_ROUND-1 — re-audit after audit-fix commit `bd60c5c1`

A fresh blind round (3 diff-only reviewers: correctness+regressions, error+security,
tests+a11y) was run against the post-fix diff.

## Results
- **error+security** — CLEAN (0 findings). Confirmed all `from_*_http` untrusted
  strings (parsed message + raw-body fallback + `gemini_prompt_blocked`) route
  through `sanitize_error_body`; 413/500/529 map to `provider` identically to the
  original; no secret leakage.
- **correctness+regressions** — **1 new confirmed (medium)**: the row-override
  re-add in `resolve()` also undid the Anthropic **thinking-active** sampling
  drop, so a thinking request with `supports_sampling_params:true` would send a
  temperature thinking forbids. FIXED: the row override now beats only the
  CAPABILITY guess (family pattern / catalog), not the per-call thinking
  reconciliation (removed the broad end-block; guarded the OpenAI reasoning strip
  with `thinking_active || !row_allows_sampling`; Anthropic thinking drop stays
  unconditional). Added tests.
- **tests+a11y** — 1 medium + 3 low, all REJECTED with rationale (see LEDGER):
  the Anthropic mock matches ziee's verbatim-moved wire struct (real-wire
  fidelity is a pre-existing, out-of-scope concern, now noted as a follow-up +
  clarified in the test); the openai_200 identity assertion is covered by the
  anthropic (`end_turn`) test; the Select aria-describedby/invalid/required props
  are never injected for these unvalidated Option toggles; the per-row help stays
  visible text next to the labeled control.

**New confirmed findings:** 1

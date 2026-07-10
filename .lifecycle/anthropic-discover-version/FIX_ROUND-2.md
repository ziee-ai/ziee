# FIX_ROUND-2 — convergence re-audit

After applying the round-1 fixes (commit 71e75f89), a fresh adversarial re-audit
re-reviewed the changed lines from every angle (correctness, edge-cases,
test-efficacy, determinism, naming-consistency, regression) specifically hunting
for defects INTRODUCED by the fixes.

Result: **no new defects.** Verified:

- `.and_then(as_str).or_else(...)` keeps a present string `name` (OpenRouter)
  winning over `display_name`; the fallback only fires on absent/null/non-string
  `name` — no OpenRouter/OpenAI/Gemini regression.
- `null_name_falls_back_to_display_name` is a genuine guard (fails if the reorder
  is reverted). The integration test is deterministic (catalog is
  `include_str!`-embedded; `claude-opus-4-8` present with `deprecated:false`; note
  substring matches the Err-branch format string exactly).

One INFORMATIONAL observation (not a defect, no change made):
`display_name_equal_to_id_or_empty_is_dropped` passes on both pre- and post-fix
code because it exercises the unchanged `.filter`, so it is not a *reorder* guard
— but it is still a valid, previously-missing coverage test for the empty/==id
filter on the `display_name` branch (audit item 3), so it is kept intentionally.

**New confirmed findings:** 0

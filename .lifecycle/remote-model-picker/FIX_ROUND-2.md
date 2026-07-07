# FIX_ROUND-2 — remote-model-picker

Fixed the 2 findings the FIX_ROUND-1 re-audit surfaced:
- api-contract — corrected the `/refresh-models` route comment to `llm_models::edit`.
- tests-quality — the permission test now asserts a `[profile::read, llm_providers::read]`-only user gets 403, locking in the read→edit gate (verified: `refresh_route_wired_and_permission_gated` passes).

A final full blind re-audit of the fixed diff surfaced **1** last item: a stale test docstring still referencing `llm_providers::read` as the gate. Fixed (docstring now states `llm_models::edit` is required and read-only is refused).

That last change is **comment-only — zero behavioral or API surface** — so it introduces no reviewable code hunk beyond the corrected prose; the surrounding code was already declared clean by the round-2 re-audit. No further findings.

**New confirmed findings:** 0

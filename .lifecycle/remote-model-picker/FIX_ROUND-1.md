# FIX_ROUND-1 — remote-model-picker

Fixed all confirmed findings from the Phase-6 blind audit:

- HIGH state-management — drawer now reads the reactive store maps unconditionally then indexes by providerId (no conditional proxy hooks).
- MEDIUM perms-authz — `POST /refresh-models` gated on `llm_models::edit` (was `llm_providers::read`); frontend button + integration-test admin perms updated.
- MEDIUM a11y ×2 — accessible names on the picker Combobox + custom-id toggle Switch.
- LOW correctness — removed the dead `supports_embeddings`-from-`input_modalities` heuristic.
- LOW perf — create_model extra provider read: rejected (no api_key decryption on this branch; cold admin path; provider_type is only obtainable via that read).

A fresh full blind re-audit of the fixed diff then surfaced **2 NEW findings** — the perm-change was not fully locked in:

1. MEDIUM tests-quality — the permission test didn't prove `llm_providers::read` alone is now insufficient (positive user held both read+edit; negative held neither), so a revert to the old gate would pass uncaught.
2. LOW api-contract — a stale route comment still said "gated by llm_providers::read".

Both are fixed in FIX_ROUND-2 (route comment corrected; the perm test now asserts a `llm_providers::read`-only user gets 403).

**New confirmed findings:** 2

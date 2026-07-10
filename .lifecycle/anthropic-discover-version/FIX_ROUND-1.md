# FIX_ROUND-1 — apply audit findings

Round 1 fixes the actionable items from the phase-6 blind audit (4 parallel
auditors: backend correctness/security, backend test quality, frontend e2e,
cross-cutting consistency). Non-defect observations (intentional test literal,
deliberate inline-POST, external endpoint) were reviewed and kept with rationale
recorded in LEDGER.jsonl — fixing them would weaken the tests or over-engineer.

Findings addressed this round:

1. **[low, correctness]** `parse_one_live_model` display_name fallback keyed on
   key-absence → a `"name": null`/non-string would short-circuit the
   `display_name` fallback. Fixed: `name.and_then(as_str).or_else(|| display_name
   .and_then(as_str))` so the fallback fires whenever `name` is not a usable
   string. (discover.rs)
2. **[medium, coverage]** No Rust integration test for the probe-failure path.
   Added `discover_anthropic_probe_failure_keeps_catalog_and_notes` — asserts a
   400 probe still yields 200 + retained catalog (`claude-opus-4-8`, source
   `catalog`) + a non-blocking fallback note.
3. **[low, coverage]** display_name==id/empty and null-name fallback were
   unit-untested. Added `display_name_equal_to_id_or_empty_is_dropped` and
   `null_name_falls_back_to_display_name`.

Verification after fixes: `cargo test --lib -p ziee discover` = 8/8 PASS;
`cargo test --test integration_tests discover_models` = 7/7 PASS.

**New confirmed findings:** 3

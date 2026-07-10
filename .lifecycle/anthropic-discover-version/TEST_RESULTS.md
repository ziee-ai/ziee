# TEST_RESULTS — phase 8

All enumerated tests run green. Logs saved under the worktree root
(`discover-int-*.log`, `e2e-fg3.log`).

- **TEST-1**: PASS — `cargo test --lib -p ziee discover` = 8/8 (incl. `anthropic_models_shape_reads_display_name`, `id_only_item_has_no_display_name`, `display_name_equal_to_id_or_empty_is_dropped`, `null_name_falls_back_to_display_name`).
- **TEST-2**: PASS — `cargo test --test integration_tests discover_models` = 7/7 (incl. `discover_anthropic_sends_version_header_and_populates_models` — the wiremock header-matcher proves `anthropic-version` is sent — and the added `discover_anthropic_probe_failure_keeps_catalog_and_notes`).
- **TEST-3**: PASS — `npm run test:e2e -- tests/e2e/llm/remote-model-picker.spec.ts --workers=1` = 3/3, including `anthropic fallback note keeps the picker enabled and selectable`. (Run via `sg docker` with `ZIEE_E2E_BASE_VITE_PORT=19000`/`ZIEE_E2E_BASE_BACKEND_PORT=19100` to avoid the host's MinIO :9000 collision — an environment issue, not a product defect; the untouched sibling specs also pass.)

Frontend gate (UI workspace touched by the e2e spec):

npm run check (ui): PASS — tsc + biome guardrails + lint:colors/settings-field + check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix all green (71 crawl values, 0 fatal; state matrix up to date; overlay gate OK).

Backend workspace: `cargo check -p ai-providers` clean; the ziee lib + integration_tests compile and run green.

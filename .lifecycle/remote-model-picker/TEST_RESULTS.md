# TEST_RESULTS — remote-model-picker

All enumerated tests implemented and passing (Phase 8, scoped to the change).

## Unit
- **TEST-1**: PASS — `discoveredModelForm.test.ts` (4 cases) via `node --test`.
- **TEST-3**: PASS — `discover.rs` `parse_live_models` (4 cases) via `cargo test --lib`.
- **TEST-4**: PASS — `provider.rs` openrouter→OpenAI dispatch via `cargo test -p ai-providers --lib`.
- **TEST-5**: PASS — `prune.rs` `decide_deprecations` (5 cases) via `cargo test --lib`.

## Integration (Postgres + wiremock /models boundary)
- **TEST-7**: PASS — `discover_models_test` (OpenRouter enrichment + 403 gate).
- **TEST-8**: PASS — `deprecation_sweep_test::refresh_flags_removed_model_and_emits_sync` (dual sync pair).
- **TEST-9**: PASS — `create_deprecated_test` (create-time catalog flag persisted).
- **TEST-12**: PASS — `deprecation_sweep_test::refresh_route_wired_and_permission_gated` (route wired + read-only 403).

## E2E (Playwright, --workers=1) — final full run: 4 passed
- **TEST-10**: PASS — `remote-model-picker.spec.ts` (picker discovers catalog, keyboard-select auto-fills, saves; custom-id toggle).
- **TEST-11**: PASS — `deprecated-model-refresh.spec.ts` (OpenRouter in type list; Deprecated badge; Refresh reconciles).

## Frontend gate
npm run check (ui): PASS

## Backend commands
- `cargo test --lib -p ziee -- discover::tests prune::deprecation_tests` → 9 passed
- `cargo test -p ai-providers --lib` → 67 passed
- `cargo test --test integration_tests llm_provider::discover_models_test llm_model::deprecation_sweep_test llm_model::create_deprecated_test -- --test-threads=2` → 4 passed
- `npx playwright test tests/e2e/llm/{remote-model-picker,deprecated-model-refresh}.spec.ts --workers=1` → 4 passed

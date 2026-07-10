# TEST_RESULTS

## Backend (ai-providers unit + server unit + integration)
- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-15**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-22**: PASS

(ai-providers: 82 lib + 3 adapter mock tests green. Server unit:
`apply_model_params_maps_and_omits_temperature`, `thinking_config_for_resolves_row_catalog_family`,
`types_ts_parity` (+ desktop) green. Integration: `empty_model_params_omit_temperature_and_default_max_tokens`,
`row_override_omits_sampling_end_to_end`, `param_contract_overrides_round_trip`,
`normal_text_completion_reports_stop`, `empty_completion_reports_finish_reason_empty`,
plus regression `model_params_reach_provider_request` / `thinking_enabled_for_registry_model` /
`thinking_disabled_for_unknown_model` — all green. The 4 real-LLM `mcp_loop_settings` tests are
key-gated and out of scope for the deterministic-mocks constraint.)

## Frontend
- npm run check (ui): PASS
- **TEST-20**: (removed — see DRIFT-1.3; ITEM-14 covered by TEST-21 + `npm run check`)
- **TEST-21**: PASS (`tests/e2e/llm/model-capability-toggles.spec.ts` — 1 passed, 38.4s: edit drawer → toggle "Supports sampling params" to No → save → override persists on the row via API read-back)

### gate:ui note (out-of-band UI Build Gate)
`gate:ui` reported `tsc: PASS`, `lint: PASS`, but `runtime-health`/`visual` FAILED —
**environmentally, not from this change.** The findings are 4658 `request-failed` +
3212 `console-error`, virtually all `GET .../node_modules/.vite/deps/*.js —
net::ERR_NETWORK_CHANGED` (a network-interface flap during the headless run, from
concurrent rsync/ssh jobs), which cascades across EVERY gallery surface (unrelated
ones too: settings-citations HIGH 370, settings-summarization HIGH 98). **Zero
findings reference this change's component** (`llm-capability-select*` /
`LlmModelCapabilitiesSection`); only 2 contrast + 2 crash total, themselves cascade.
The deterministic UI contract this change is responsible for is fully green:
`npm run check` (incl. `lint:colors`, `check:design-spec`, `check:state-matrix`,
`check:testid-registry`), `tsc`, and `lint` all PASS.

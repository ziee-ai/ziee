# TESTS — remote-model-picker

Every ITEM is covered by ≥1 TEST; UI items also carry an e2e spec.

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/ui/src/modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer.tsx` — asserts: selecting a DiscoveredModel maps supports_* → capability toggles + context_length onto form values, and the custom-id toggle swaps the picker for a plain input.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts` — asserts: `discoverModels` calls the client and stores models/notes/loading/error.
- **TEST-3** (tier: unit) [covers: ITEM-7] file: `src-app/server/src/modules/llm_provider/handlers/discover.rs` — asserts: the rich-field parser maps context_length / input_modalities→vision / supported_parameters→tools and drops pricing.
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/server/ai-providers/src/provider.rs` — asserts: `Provider::new("openrouter", ..)` dispatches to the OpenAI-compatible client.
- **TEST-5** (tier: unit) [covers: ITEM-8] file: `src-app/server/src/modules/llm_model/prune.rs` — asserts: `sweep_provider_once` flips is_deprecated only on a successful non-empty live set, clears on reappearance, and never flags on an empty/failed fetch.
- **TEST-6** (tier: unit) [covers: ITEM-9] file: `src-app/server/src/modules/llm_model/handlers/models.rs` — asserts: create_model sets is_deprecated from a catalog-deprecated id via the provider-type lookup.
- **TEST-7** (tier: integration) [covers: ITEM-7, ITEM-5] file: `src-app/server/tests/llm_provider/discover_models_test.rs` — asserts: discover-models against a mock OpenAI-compat `/models` and a keyless OpenRouter-shaped `/models` returns enriched capabilities; perm-gate 403.
- **TEST-8** (tier: integration) [covers: ITEM-8, ITEM-4] file: `src-app/server/tests/llm_model/deprecation_sweep_test.rs` — asserts: the single-provider reconcile endpoint flags a model the mock dropped between calls, emits the dual permission-scoped sync pair (LlmModel + UserLlmProvider) via SyncProbe, and is llm_models-permission gated (403 without).
- **TEST-9** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/llm_model/create_deprecated_test.rs` — asserts: adding a catalog-deprecated model persists is_deprecated=true.
- **TEST-10** (tier: e2e) [covers: ITEM-1, ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/05-llm/remote-model-picker.spec.ts` — asserts: open add-remote drawer → picker lists discovered models → select auto-fills capabilities → save; the custom-id toggle path works.
- **TEST-11** (tier: e2e) [covers: ITEM-4, ITEM-6] file: `src-app/ui/tests/e2e/05-llm/deprecated-model-refresh.spec.ts` — asserts: OpenRouter appears in the provider-type list; a deprecated model shows the badge; "Refresh models" flags a vanished model.
- **TEST-12** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/llm_model/deprecation_sweep_test.rs` — asserts: the reconcile route is registered/reachable (routes wiring live) and returns the updated model list, proving the sweep entrypoint is wired into the running server.

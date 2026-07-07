# PLAN — remote-model-picker

## Context

Adding a model to a **remote** LLM provider is pure free-text today
(`AddRemoteLlmModelDrawer` → `BASIC_MODEL_FIELDS` → a plain `<Input>` "Model ID";
capabilities/context hand-entered or defaulted). Typos silently produce a model
that 400s at inference.

The backend discovery is already built and wired but unconsumed by the UI:
`GET /api/llm-providers/{id}/discover-models` (`handlers/discover.rs`) returns a
3-layer result (bundled keyless catalog + live SSRF-hardened `GET {base_url}/models`
+ notes) as `DiscoverModelsResponse { provider_type, models: DiscoveredModel[],
notes }`, already in the generated TS types. `llm_models` already has an unused
`is_deprecated` column and a `capabilities JSONB` (incl. `context_length`);
nothing sets `is_deprecated` or detects a vanished model. Approved scope:
**background sweep**, **first-class OpenRouter**, **auto-populate
capabilities+context (no pricing)**.

## Items

- **ITEM-1**: Replace the free-text "Model ID" in `AddRemoteLlmModelDrawer` with a discovery-backed searchable `Combobox` picker populated from `discoverModels`, showing per-option context-length hint, source tag, and a Deprecated badge; plus a "custom model id" toggle that swaps to a plain `<Input>` fallback (Combobox is not creatable).
- **ITEM-2**: On picker selection, auto-populate the form — `display_name` and `capabilities` mapped from the chosen `DiscoveredModel` (`supports_chat→chat`, `supports_embeddings→text_embedding`, `supports_vision→vision`, `supports_tool_use→tools`, `context_length→capabilities.context_length`) — replacing the currently-hardcoded `capabilities` object in `onValid`; all fields stay user-overridable.
- **ITEM-3**: Add a `discoverModels(providerId)` action to `LlmProvider.store.ts` calling `ApiClient.LlmProvider.discoverModels({ provider_id })`, with loading/error state consumed by the drawer.
- **ITEM-4**: Render a "Deprecated / Unavailable" badge on `is_deprecated` model rows in `LlmModelsSection`, and add a "Refresh models" button that calls the single-provider reconcile (ITEM-8) then reloads; deprecated rows offer Remove (existing `deleteLlmModel`) and a "pick replacement" affordance re-opening the picker.
- **ITEM-5**: Add `openrouter` as a first-class `provider_type` — new migration extending the `llm_providers.provider_type` CHECK to include `'openrouter'` and seeding a built-in row (`https://openrouter.ai/api/v1`); add `"openrouter"` to `validate_provider_type`; route `"openrouter"` → `OpenAIProvider` in `ai-providers/src/provider.rs`.
- **ITEM-6**: Add `openrouter` to the UI `PROVIDER_TYPES` list (`LlmProviderDrawer.tsx`) and a `PROVIDER_ICONS` entry (`constants.tsx`).
- **ITEM-7**: Enrich `handlers/discover.rs::fetch_v1_models` to parse richer per-model fields when present (`context_length`, `architecture.input_modalities`→vision, `supported_parameters`→tools) into `DiscoveredModel` instead of id-only; OpenRouter's `/models` is fetched keyless; pricing is parsed-and-dropped. No new `DiscoveredModel` field ⇒ no OpenAPI/type change.
- **ITEM-8**: Add a deprecation-sweep job (folded into the existing `llm_model/prune.rs` loop — already 6h tick, best-effort, boot reconcile) that per remote provider runs discovery and, only on a successful non-empty live fetch, sets `is_deprecated=true` on saved models absent from the live set (or catalog-deprecated) and clears it on reappearance, emitting the dual permission-scoped sync pair (`SyncEntity::LlmModel`/`LlmModelsRead` + `SyncEntity::UserLlmProvider`/`UserLlmProvidersRead`, `origin=None`); plus a `set_model_deprecated(model_id,bool)` repo method and a `sweep_provider_once` reconcile fn reused by ITEM-4's handler.
- **ITEM-9**: In `create_model`, fetch the provider to get `provider_type` (the request carries only `provider_id`), then if `registry_lookup(provider_type, name).deprecated` is true call `set_model_deprecated(model.id, true)` after `Repos.llm_model.create` — so a known-deprecated pick is flagged immediately.
- **ITEM-10**: Wire the sweep into the running server — extend the existing `llm_model/mod.rs::init()` spawn (which already runs `run_prune_loop`) so the folded sweep job executes each tick; expose the reconcile route in `llm_model/routes.rs`.

## Files to touch

- `src-app/ui/src/modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer.tsx` (ITEM-1,2)
- `src-app/ui/src/modules/llm-provider/components/llm-models/shared/LlmModelParameterField.tsx` (ITEM-1)
- `src-app/ui/src/modules/llm-provider/stores/LlmProvider.store.ts` (ITEM-2,3)
- `src-app/ui/src/modules/llm-provider/components/LlmModelsSection.tsx` (ITEM-4)
- `src-app/ui/src/modules/llm-provider/components/LlmProviderDrawer.tsx` (ITEM-6)
- `src-app/ui/src/modules/llm-provider/constants.tsx` (ITEM-6)
- `src-app/server/migrations/00000000000132_add_openrouter_provider_type.sql` (ITEM-5)
- `src-app/server/src/modules/llm_provider/utils.rs` (ITEM-5)
- `src-app/server/ai-providers/src/provider.rs` (ITEM-5)
- `src-app/server/src/modules/llm_provider/handlers/discover.rs` (ITEM-7)
- `src-app/server/src/modules/llm_model/prune.rs` (ITEM-8)
- `src-app/server/src/modules/llm_model/repository.rs` (ITEM-8)
- `src-app/server/src/modules/llm_model/handlers/models.rs` (ITEM-8, ITEM-9)
- `src-app/server/src/modules/llm_model/routes.rs` (ITEM-10)
- `src-app/server/src/modules/llm_model/mod.rs` (ITEM-10)

## Patterns to follow

- **Sweep loop** → extend the existing `src-app/server/src/modules/llm_model/prune.rs` (a 6h best-effort boot-reconcile loop with N jobs per tick; itself mirrors `mcp/tool_calls/prune.rs`). Add the sweep as another per-tick job — do not invent a parallel loop.
- **Discovery/live fetch** → extend the existing `handlers/discover.rs::fetch_v1_models`, keeping its `url_validator` SSRF guards verbatim.
- **Provider-type addition** → mirror how the 9 existing types are threaded (migration CHECK + seed in `..0003`, `validate_provider_type`, `provider.rs` dispatch).
- **Sync emit** → the Realtime Sync convention (`sync_publish`, dual permission-scoped emit) exactly as `create_model` already does.
- **Picker UI** → the `Combobox` kit component + `FormField` binding already used by the provider-type `Select`; badges match the capability-chip style in `LlmModelsSection`.
- **Settings-card style** → the existing `LlmModelsSection` card.

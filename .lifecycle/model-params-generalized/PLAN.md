# PLAN — Generalized, dynamic provider-adapter for model params (request + response)

Replaces PR #122's Anthropic-only hardcoded self-heal (reverted in the branch's
first commit) with a provider-agnostic adapter that is correct **by
construction** for BOTH request and response. A model's param contract is
**resolved** (editable DB row → curated catalog → O(families) provider policy →
conservative default), with request thinking/reasoning semantics honored on top.
No error-driven self-heal. Full design: `/home/khoi/.claude/plans/read-data-khoi-home-workspace-ziee-ziee-valiant-pie.md`.

## Items

- **ITEM-1**: New `ai-providers/src/param_policy.rs` — declarative request contract: `UnifiedParam`, `MaxTokensField{key()}`, `ProviderFamily`, `ResolvedParams{allows()}`, `ModelParamContract`, and `resolve(family, model_id, req, contract)` layering (family base → contract capability → model-family pattern → thinking/reasoning reconciliation). Family-pattern predicates: `openai_reasoning_family`, `openai_requires_non_streaming`, `anthropic_sampling_restricted`, and `thinking_capability(family, model_id, contract)`. Pure, no I/O; `resolve` consults `model_registry::lookup` for the catalog layer.
- **ITEM-2**: Anthropic `build_request_body` consumes `ResolvedParams` (emit temperature/top_p/top_k/`max_tokens` via `rp.allows`/`rp.max_tokens_field`); delete `sampling_restricted` (superseded by `resolve`). Keep the "at most one of temperature/top_p" body-shape rule local.
- **ITEM-3**: OpenAI `build_request_body` consumes `ResolvedParams`; delete the `MODELS_REQUIRING_NON_STREAMING` const (moved to `param_policy::openai_requires_non_streaming`); the gpt-5 non-streaming quirk is driven by `rp.disable_stream`; `max_tokens` vs `max_completion_tokens` via `rp.max_tokens_field`; `reasoning_effort` via `rp.use_reasoning_effort`.
- **ITEM-4**: Gemini `build_request_body` consumes `ResolvedParams` (first-time capability gating of `generationConfig` sampling fields); its `stream_chat` serializes the typed request to `serde_json::Value` before send so it shares the generic driver.
- **ITEM-5**: `FinishReason` enum (`Stop|Length|ToolCalls|ContentFilter|Refusal|Other`) + per-provider `&str -> FinishReason` tables + `as_canonical_str()`; `StreamChatChunk.finish_reason` stays `Option<String>` but carries the canonical value.
- **ITEM-6**: Typed per-provider error parsers in `error.rs`: `parse_anthropic_error` (relocated envelope parse), `parse_openai_error` (`{error:{param,code,message}}`), `parse_gemini_error` (`{error:{status,message}}`) → common `ProviderError` (each sanitized). Closes the OpenAI/Gemini typed-error gap.
- **ITEM-7**: New `ai-providers/src/providers/sse.rs` — generic SSE driver owning the scaffolding (decoder, delimiter split, `data:`/`[DONE]`, `MAX_SSE_BUFFER_BYTES`, `Network(e)`, skip-unparseable), parameterized by a `ResponseAdapter` trait each provider implements (`sse_delimiter`, `map_event`). The 3 copy-pasted SSE loops collapse into `sse::drive(response, self)`; OpenAI `non_streaming_to_stream` becomes a first-class "single JSON body → fan-out" response mode. Per-provider delta behavior preserved verbatim.
- **ITEM-8**: Add `ChatRequest.model_caps: Option<ModelParamContract>` (`#[serde(default, skip_serializing_if = "Option::is_none")]`); `resolve` treats it as the top-priority capability source; `None` ⇒ ai-providers falls back to catalog+family (self-sufficient for non-chat callers).
- **ITEM-9**: Extend the server DB `ModelCapabilities` (`llm_model/models.rs`) with `supports_thinking: Option<bool>`, `thinking_style: Option<String>`, `supports_sampling_params: Option<bool>` (schema-free JSONB, **no migration**); add a mapper `db ModelCapabilities -> ai_providers::ModelParamContract`.
- **ITEM-10**: Chat wiring (`chat/core/services/streaming.rs`): populate `ChatRequest.model_caps` from `model.capabilities`; `thinking_config_for` resolves via `param_policy::thinking_capability` (row → catalog → family) instead of catalog-only; re-remove the forced `temperature 0.7` in `apply_model_params` (`req.temperature = p.temperature`), keep `max_tokens.or(Some(8192))`.
- **ITEM-11**: Finish-reason canonicalization ripple (`chat/core/types/streaming.rs` + call sites): ensure downstream continue/stop logic and finish-reason-literal comparisons use the canonical vocabulary; adjust affected server tests (`tests/chat/stub_chat_tier2_test.rs`, `tests/mcp/mcp_loop_settings_test.rs`).
- **ITEM-12**: Model create path (`llm_model/handlers/models.rs`): pre-fill inferred capability defaults (`supports_thinking`/`thinking_style`/`supports_sampling_params`) onto a new row at create time (catalog+family inference) when the request omits them, so the row is the editable source of truth going forward.
- **ITEM-13**: Re-add the curated `claude-sonnet-5` entry (and Claude-5 family flags) to `data/known_models.json` — legitimate catalog data (the catalog layer), not #122 hardcoding.
- **ITEM-14**: UI capability toggles — add `supports_thinking` / `thinking_style` / `supports_sampling_params` controls to `LlmModelCapabilitiesSection`, wire Add/Edit drawers + `discoveredModelForm` inferred pre-fill, in `src-app/ui` and mirror `src-app/desktop/ui`.
- **ITEM-15**: `just openapi-regen` → regenerated `openapi.json` + `api-client/types.ts` in both ui workspaces (mechanical; reflects `ChatRequest.model_caps` + the new capability fields).

## Files to touch

- `src-app/server/ai-providers/src/param_policy.rs` (new)
- `src-app/server/ai-providers/src/providers/sse.rs` (new)
- `src-app/server/ai-providers/src/providers/{anthropic,openai,gemini}.rs`
- `src-app/server/ai-providers/src/providers/mod.rs` (driver wiring if needed)
- `src-app/server/ai-providers/src/error.rs`
- `src-app/server/ai-providers/src/models/chat.rs` (`FinishReason`, `ChatRequest.model_caps`)
- `src-app/server/ai-providers/src/model_registry.rs` (catalog layer stays)
- `src-app/server/ai-providers/data/known_models.json`
- `src-app/server/ai-providers/src/lib.rs` (exports)
- `src-app/server/src/modules/llm_model/models.rs` (DB `ModelCapabilities` + mapper)
- `src-app/server/src/modules/llm_model/handlers/models.rs` (create-time inference)
- `src-app/server/src/modules/chat/core/services/streaming.rs`
- `src-app/server/src/modules/chat/core/types/streaming.rs`
- `src-app/ui/src/modules/llm-provider/**` (capabilities section + Add/Edit drawers + discoveredModelForm)
- `src-app/desktop/ui/src/modules/llm-provider/**` (mirror)
- `src-app/{ui,desktop/ui}/src/api-client/types.ts` + `openapi/openapi.json` (regenerated)

## Patterns to follow

- **Capability resolution (row preferred, catalog fallback)**: mirror `src-app/server/src/modules/file/available_files.rs:185-206`, which already prefers the DB `capabilities` over the catalog for `supports_tool_use`.
- **Declarative catalog + pure request builder**: mirror `ai-providers/src/model_registry.rs` (`OnceLock` + `include_str!`) and the existing pure, unit-tested `build_request_body` fns.
- **Shared streaming infra**: extend `ai-providers/src/providers/mod.rs` (`http_client`, `Utf8StreamDecoder`, `MAX_SSE_BUFFER_BYTES`) — the new `sse.rs` lives alongside them.
- **Typed provider errors**: mirror `error.rs::from_anthropic_error` (both fields via `sanitize_error_body`) generalized to OpenAI/Gemini.
- **UI capability toggles**: mirror the existing `LlmModelCapabilitiesSection` Switch-bound-to-`capabilities.*` pattern; Add/Edit drawer wiring mirrors the existing `vision`/`tools` fields.
- **Deterministic provider HTTP tests**: mirror the in-file raw-`TcpStream` `spawn_mock`/`http_200_sse`/`http_400` harness in `anthropic.rs`'s test module.
- **OpenAPI regen**: `just openapi-regen` (server + desktop specs; `emit_ts.rs` golden parity test guards it).

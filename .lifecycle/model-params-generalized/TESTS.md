# TESTS — enumerated up front

Tiers mirror the codebase: unit = in-source `#[cfg(test)]` (ai-providers/server)
or vitest (`*.test.ts`); integration = `src-app/server/tests/<module>/`; e2e =
`src-app/ui/tests/e2e/`. The 4 request scenarios are **pure** (assert the built
body from `resolve` + `build_request_body`, no HTTP). Response/error parsing is
exercised by feeding synthetic SSE bytes to the driver (unit) + one full
`stream_chat` over the existing raw-`TcpStream` mock (integration, in-crate).

## Request contract + capability resolution

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/param_policy.rs` — asserts: `resolve` precedence — row-override (`model_caps`) beats catalog beats family-pattern beats conservative default, per capability signal.
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/param_policy.rs` — asserts: field-name map per family — OpenAI non-reasoning→`max_tokens`, reasoning→`max_completion_tokens`, gpt-5→`max_completion_tokens`+`disable_stream`; Anthropic→`max_tokens`; Gemini→`maxOutputTokens`.
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/param_policy.rs` — asserts: reconciliation — thinking active ⇒ temperature/top_p/top_k ineligible; OpenAI reasoning ⇒ sampling+penalties ineligible + `use_reasoning_effort`; catalog `supports_sampling_params=false` ⇒ sampling ineligible without thinking.
- **TEST-4** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/param_policy.rs` — asserts: family patterns generalize with NO catalog entry — `o5-mini` inferred reasoning; `claude-opus-4-9` inferred sampling-restricted; a plain chat id passes sampling through.
- **TEST-5** (tier: unit) [covers: ITEM-1, ITEM-8] file: `src-app/server/ai-providers/src/param_policy.rs` — asserts: graceful degrade — `model_caps = None` + unknown model + no pattern ⇒ user-set params pass through, nothing injected; `model_caps = None` falls back to catalog+family (self-sufficient).
- **TEST-6** (tier: unit) [covers: ITEM-2] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: `build_request_body` emits at-most-one of temperature/top_p when allowed; omits all sampling when `resolve` disallows (sonnet-5 via catalog; opus-class via pattern); `max_tokens` always present.
- **TEST-7** (tier: unit) [covers: ITEM-3] file: `src-app/server/ai-providers/src/providers/openai.rs` — asserts: reasoning model → `max_completion_tokens`+`reasoning_effort`, no temperature/top_p/penalties; non-reasoning → `max_tokens`+sampling; gpt-5 → `stream:false`(`disable_stream`)+`max_completion_tokens`, no sampling/seed/stop.
- **TEST-8** (tier: unit) [covers: ITEM-4] file: `src-app/server/ai-providers/src/providers/gemini.rs` — asserts: `generationConfig` sampling fields gated by `resolve` (first-time gating); `maxOutputTokens` used; serialize-to-`Value` happy-path body is byte-identical to the pre-refactor typed send (snapshot).
- **TEST-9** (tier: unit) [covers: ITEM-13] file: `src-app/server/ai-providers/src/model_registry.rs` — asserts: `lookup("anthropic","claude-sonnet-5").supports_sampling_params == Some(false)`; a dated `claude-sonnet-5-*` SKU resolves to the base entry.

## Response contract (finish-reason, typed errors, SSE driver)

- **TEST-10** (tier: unit) [covers: ITEM-5] file: `src-app/server/ai-providers/src/models/chat.rs` — asserts: per-provider `&str→FinishReason` tables + `as_canonical_str` (Anthropic `end_turn→stop`/`tool_use→tool_calls`/`max_tokens→length`; OpenAI passthrough; Gemini `STOP→stop`/`MAX_TOKENS→length`/`SAFETY→content_filter`).
- **TEST-11** (tier: unit) [covers: ITEM-6] file: `src-app/server/ai-providers/src/error.rs` — asserts: `parse_openai_error`/`parse_anthropic_error`/`parse_gemini_error` map their wire shapes to the correct `ProviderError` variant, each sanitized/bounded.
- **TEST-12** (tier: unit) [covers: ITEM-7] file: `src-app/server/ai-providers/src/providers/sse.rs` — asserts: the generic driver over a synthetic in-memory byte stream yields the same `StreamChatChunk` sequence for each provider's `map_event` (text/thinking/signature/redacted/tool-use deltas, usage, canonical finish, in-stream error), incl. the OpenAI index-freeze + Gemini uuid-tool-id + non-streaming fan-out modes — byte-identical vs pre-refactor.
- **TEST-13** (tier: integration) [covers: ITEM-7, ITEM-6] file: `src-app/server/ai-providers/tests/adapter_response_test.rs` — asserts: a full `stream_chat` over the raw-`TcpStream` mock returns unified deltas + canonical finish for a 200 SSE response, and a typed `ProviderError` for a 400 error body, for ≥2 providers (Anthropic + OpenAI).

## Backend wiring (DB caps, chat, create, finish-reason ripple)

- **TEST-14** (tier: unit) [covers: ITEM-9] file: `src-app/server/src/modules/llm_model/models.rs` — asserts: DB `ModelCapabilities{supports_thinking,thinking_style,supports_sampling_params}` maps to `ai_providers::ModelParamContract`; unset fields → `None`.
- **TEST-15** (tier: unit) [covers: ITEM-10] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: `apply_model_params` omits temperature when unset (no forced 0.7) and defaults `max_tokens` to 8192; `thinking_config_for` prefers the row cap over catalog over family.
- **TEST-16** (tier: integration) [covers: ITEM-10] file: `src-app/server/tests/chat/stub_chat_tier2_test.rs` — asserts: empty model params send NO temperature (omitted) and `max_tokens=8192` on the wire to the stub.
- **TEST-17** (tier: integration) [covers: ITEM-9, ITEM-12] file: `src-app/server/tests/llm_model/capabilities_infer_test.rs` — asserts: creating a model with explicit capability flags round-trips them; creating one WITHOUT flags bakes inferred defaults (e.g. a sonnet-5 id → `supports_sampling_params=false`) onto the row.
- **TEST-18** (tier: integration) [covers: ITEM-8, ITEM-1] file: `src-app/server/tests/chat/stub_chat_tier2_test.rs` — asserts: a model row with `capabilities.supports_sampling_params=false` (threaded via `ChatRequest.model_caps`) omits sampling on the wire even for a model the catalog/family would allow (row-override wins end-to-end).
- **TEST-19** (tier: integration) [covers: ITEM-11] file: `src-app/server/tests/mcp/mcp_loop_settings_test.rs` — asserts: canonical finish-reason values (`tool_calls`/`stop`) still drive the chat loop's continue-vs-stop decision correctly (no regression from canonicalization).

## Frontend

- **TEST-20** (tier: unit) [covers: ITEM-14] file: `src-app/ui/src/modules/llm-provider/components/llm-models/discoveredModelForm.test.ts` — asserts: `mapDiscoveredModelToForm` carries the new capability toggles (`supports_thinking`/`thinking_style`/`supports_sampling_params`) with inferred defaults.
- **TEST-21** (tier: e2e) [covers: ITEM-14] file: `src-app/ui/tests/e2e/05-llm/model-capability-toggles.spec.ts` — asserts: a user adds/edits a model, toggles the sampling-restricted capability, saves, reloads, and the toggle persists.
- **TEST-22** (tier: unit) [covers: ITEM-15] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: the existing `types_ts_parity` golden test passes after regen — `types.ts` matches the committed `openapi.json` (guards the mechanical regen of the new fields).

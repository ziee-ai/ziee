# PLAN — Model sampling-parameter constraints (Anthropic temperature/thinking)

Fixes two reproduced HTTP 400s where ziee sends sampling params the Anthropic model rejects:
(1) a manually-added `claude-sonnet-5` is unknown to the registry so `temperature` is sent and the
real model 400s; (2) a registered thinking-enabled model (`claude-sonnet-4-6`) is force-sent
`temperature: 0.7` while adaptive thinking is active → *"temperature may only be set to 1 when
thinking is enabled."* Root cause: request assembly never reconciles `temperature`/`top_p`/`top_k`
against (a) whether the model accepts sampling params and (b) whether thinking is active.

## Items

- **ITEM-1**: In `anthropic.rs::build_request_body`, compute a `thinking_active` flag up front (thinking present with mode `Adaptive` or `Enabled`) and gate the whole sampling block on `!sampling_restricted && !thinking_active`, so `temperature`/`top_p`/`top_k` are omitted whenever thinking is active (Anthropic defaults temperature to 1). Applies to any thinking-enabled Anthropic model. Fixes failure #2.
- **ITEM-2**: Add `claude-sonnet-5` to the `anthropic` list in `known_models.json`, mirroring the `claude-opus-4-8` entry (`supports_sampling_params: false`, `supports_thinking: true`, `thinking_style: "adaptive"`, `context_length: 1000000`, vision/tool-use true). Prefix-tolerant lookup resolves dated SKUs. Fixes failure #1.
- **ITEM-3**: In `anthropic.rs::stream_chat`, on an initial-POST 400 whose Anthropic `invalid_request_error` message names a sampling param (`temperature`/`top_p`/`top_k`/"sampling"), strip those keys from the built JSON body and retry the POST once; if the retry also fails or the 400 is unrelated, return a clean `ProviderError` built from the parsed Anthropic `type`+`message` (via `from_anthropic_error`) instead of the raw JSON blob. Keep the repair decision in pure, unit-testable helpers. Durable self-heal for future model changes.
- **ITEM-4**: In `streaming.rs::apply_model_params`, drop the `.or(Some(0.7))` fallback so `req.temperature = p.temperature` — `temperature` is sent only when genuinely configured on the model row. Keep the `max_tokens` default. Update the `apply_model_params_maps_and_defaults` unit test.

## Files to touch

- `src-app/server/ai-providers/src/providers/anthropic.rs` — ITEM-1 (`build_request_body`), ITEM-3 (`stream_chat` 400 handling + repair helpers) + their `#[cfg(test)]`.
- `src-app/server/ai-providers/data/known_models.json` — ITEM-2 (add `claude-sonnet-5` entry).
- `src-app/server/ai-providers/src/model_registry.rs` — ITEM-2 `#[cfg(test)]` (new registry assertions).
- `src-app/server/src/modules/chat/core/services/streaming.rs` — ITEM-4 (`apply_model_params`) + update its `#[cfg(test)]`.
- (Possibly `src-app/server/ai-providers/src/error.rs` — only if ITEM-3 needs a new constructor; expected reuse of existing `from_anthropic_error`, no change anticipated.)

## Patterns to follow

- **Request-body assembly (ITEM-1, ITEM-3)** — mirror the existing `anthropic.rs::build_request_body` idioms: `serde_json::Value` mutation via `body["key"] = json!(...)`, the existing `sampling_ok` gate at `:594`, and the existing thinking-block `match request.thinking` at `:625`. Reuse `ThinkingMode` from `crate::models`.
- **Registry entry (ITEM-2)** — mirror the shape of the existing `claude-opus-4-8` object in `data/known_models.json` (same keys/order); mirror the existing registry tests in `model_registry.rs` (`opus_47_thinking_adaptive_and_sampling_restricted`, `dated_and_aliased_ids_resolve_to_base_entry`).
- **Error mapping (ITEM-3)** — reuse `ProviderError::from_anthropic_error` (already maps `invalid_request_error` → clean `InvalidRequest(message)`); parse the Anthropic error envelope `{"error":{"type","message"}}` the same way the SSE-error path does at `anthropic.rs:738-744`.
- **Model-param mapping (ITEM-4)** — mirror the existing `apply_model_params` in `streaming.rs:1082` (the surrounding `.or(Some(default))` idiom, minus the temperature default) and its existing unit test.

# TESTS — model-param-constraints

Backend-only diff (Rust); no UI workspace touched, so all tiers are `unit`/`integration` and no
`e2e` is required. Tests mirror the existing tier pattern: in-source `#[cfg(test)]` unit tests in
`anthropic.rs` / `model_registry.rs` / `streaming.rs`. Mock only the external boundary (the Anthropic
HTTP endpoint) where an integration test exercises the retry round-trip.

## Unit

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: with adaptive thinking active on a sampling-allowed model (`claude-3-5-sonnet`) and temperature/top_p/top_k all set, `build_request_body` omits `temperature`, `top_p`, and `top_k` (the failure-#2 fix).
- **TEST-2** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: with `Enabled` (budget) thinking on a sampling-allowed model + temperature set, `build_request_body` omits the sampling block; the thinking block is still emitted.
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: with thinking `None`/`Disabled` on a sampling-allowed model + temperature set, `build_request_body` still emits `temperature` (no regression to the allowed+no-thinking path).
- **TEST-4** (tier: unit) [covers: ITEM-2] file: `src-app/server/ai-providers/src/model_registry.rs` — asserts: `lookup("anthropic","claude-sonnet-5")` returns `supports_sampling_params: Some(false)`, `supports_thinking: Some(true)`, `thinking_style: Some("adaptive")`.
- **TEST-5** (tier: unit) [covers: ITEM-2] file: `src-app/server/ai-providers/src/model_registry.rs` — asserts: a dated SKU `claude-sonnet-5-20260xxx` resolves (prefix-tolerant) to the bare `claude-sonnet-5` entry with `supports_sampling_params: Some(false)`.
- **TEST-6** (tier: unit) [covers: ITEM-2, ITEM-1] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: `build_request_body` for `claude-sonnet-5` (no thinking) omits `temperature`/`top_p`/`top_k` because the new registry entry makes `sampling_restricted` true (end-to-end of registry gate + assembly; the failure-#1 fix).
- **TEST-7** (tier: unit) [covers: ITEM-3] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: the pure repair predicate returns true for an Anthropic 400 body whose message names `temperature` (and for `top_p`/`top_k`/"sampling"), and false for an unrelated `invalid_request_error` message.
- **TEST-8** (tier: unit) [covers: ITEM-3] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: the strip helper removes `temperature`/`top_p`/`top_k` keys from a built body `serde_json::Value` while leaving `model`/`messages`/`thinking` intact.
- **TEST-9** (tier: unit) [covers: ITEM-3] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: parsing an Anthropic 400 error envelope `{"error":{"type":"invalid_request_error","message":"..."}}` yields a clean `ProviderError::InvalidRequest(message)` (not the raw JSON blob) via `from_anthropic_error`.
- **TEST-10** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/chat/core/services/streaming.rs` — asserts: `apply_model_params` maps a configured `temperature` through unchanged, and with `ModelParameters::default()` (unset) leaves `req.temperature == None` (the 0.7 default is gone) while `max_tokens` still defaults to 8192 (updated `apply_model_params_maps_and_defaults`).

## Integration

- **TEST-11** (tier: integration) [covers: ITEM-3] file: `src-app/server/ai-providers/src/providers/anthropic.rs` — asserts: against a loopback mock Anthropic endpoint that 400s on the first request when `temperature` is present and 200-streams when it is absent, `stream_chat` strips the sampling param, retries once, and yields a successful stream (self-heal round-trip). Runs via `base_url` override to the mock; if a same-crate `#[tokio::test]` mock server is impractical here, this drops to a scoped unit test of the retry decision + body mutation (TEST-7/TEST-8) and is noted as such in TEST_RESULTS.

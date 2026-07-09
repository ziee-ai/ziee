# PLAN_AUDIT — model-param-constraints

Audit of PLAN.md ITEMs against the actual codebase before writing code.

## Breakage risk

Existing `build_request_body` unit tests (`anthropic.rs` mod tests) were checked against ITEM-1:
- `adaptive_thinking_shape_and_effort` — `claude-opus-4-7` (restricted) + adaptive thinking, **no
  temperature set** and no temperature assertion → unaffected.
- `allowed_model_sends_temperature_not_both` — `claude-3-5-sonnet` (unknown → allowed) + temperature
  0.5, **no thinking set** → `thinking_active` is false, temperature still emitted → unaffected.
- `opus_47_omits_sampling_params`, `disabled_thinking_omits`, `enabled_thinking_uses_budget` →
  unaffected (restricted model or no sampling assertion).
So ITEM-1 breaks **no** existing test.

ITEM-4: the only runtime caller of `apply_model_params` is `streaming.rs:371` (the
`workflow/runner.rs:1216` reference is a comment, not a call). The only assertion on the 0.7 default
is `apply_model_params_maps_and_defaults` (`streaming.rs:2025`), which ITEM-4 updates. Behavior
delta: a sampling-**allowed** Anthropic model with no configured temperature now omits `temperature`
(provider applies its own default) instead of sending 0.7 — benign and the intended cleanup; other
providers already treat `None` as "omit".

ITEM-3 changes only the initial-POST error path. Non-400 responses and the success path are
untouched. The `serde_json::Value` body is cloneable and the reqwest client is reusable, so a
single retry POST is safe. Risk: must not double-consume the response or loop more than once — the
retry is guarded by a "not yet retried" flag.

ITEM-2 is purely additive to `known_models.json`; no existing entry changes. Registry tests assert
specific models exist (not counts), so adding one is safe.

## Pattern conformance

- ITEM-1 / ITEM-3 mirror the existing `build_request_body` `serde_json::Value` idioms and reuse
  `crate::models::ThinkingMode` + the existing `sampling_ok` gate.
- ITEM-2 mirrors the `claude-opus-4-8` object shape and the existing registry tests
  (`opus_47_thinking_adaptive_and_sampling_restricted`, `dated_and_aliased_ids_resolve_to_base_entry`).
- ITEM-3 reuses `ProviderError::from_anthropic_error` (already maps `invalid_request_error` → clean
  `InvalidRequest(message)`) and parses the same `{"error":{"type","message"}}` envelope the SSE
  error path uses (`anthropic.rs:738-744`). No new `ProviderError` variant needed.
- ITEM-4 mirrors the surrounding `.or(Some(default))` idiom in `apply_model_params`.

## Migration collisions

None. No item adds a SQL migration; latest migration is `132`
(`add_openrouter_provider_type.sql`). `known_models.json` is a compile-time data file baked via
`include_str!`, not a migration.

## OpenAPI regen

None required. No request/response schema type changes — `ChatRequest`/`ModelParameters` are internal
`ai-providers`/server types; ITEM-4 changes runtime behavior, not the emitted OpenAPI/`types.ts`. So
`just openapi-regen` is not needed and the diff stays backend-only (no UI gates).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — mirrors existing `build_request_body`/`sampling_ok` idioms; breaks no existing test; `thinking_active` gate is additive.
- **ITEM-2** — verdict: PASS — additive registry entry mirroring `claude-opus-4-8`; no migration, no schema change.
- **ITEM-3** — verdict: CONCERN — correct approach (reuse `from_anthropic_error`, single guarded retry), but the retry plumbing must not loop more than once nor double-read the response body; covered by tests + phase-6 concurrency/error-handling angles.
- **ITEM-4** — verdict: PASS — single call site; only the one unit test asserts the 0.7 default and it is updated; behavior delta is the intended cleanup.

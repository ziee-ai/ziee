# PLAN_AUDIT — audited against the codebase

## Breakage risk

- **`ChatRequest` new field (`model_caps`)**: additive `Option` with `#[serde(default, skip_serializing_if)]`. `ChatRequest` is built with `..Default::default()` at every site (`streaming.rs:371`, title, summarizer, memory, workflow, mcp/sampling). Adding a field requires `Default` — `ChatRequest` already derives `Default`, and an `Option` field defaults to `None`. No caller breaks. Non-chat callers pass `None` ⇒ ai-providers falls back to catalog+family. ✅
- **`AIProvider` trait signature preserved** (`stream_chat(api_key, base_url, request)`). The generic SSE driver is an internal refactor of each `stream_chat` body — no trait change, so `Box<dyn AIProvider>` consumers (`provider.rs`, `provider_routing.rs`, `MockProvider` in `llm_provider_files`) are unaffected.
- **`StreamChatChunk` shape preserved** — `finish_reason` stays `Option<String>`; only its *value* is canonicalized. Ripple limited to `chat/core/types/streaming.rs::from_ai_providers_delta` (delta mapping, unaffected) and the finish-reason literal comparisons in the chat loop + two test files (ITEM-11). Risk: canonical mapping must preserve the loop's continue-on-tool-calls / stop semantics — covered by ITEM-11 tests.
- **SSE loop refactor** (ITEM-7): the highest-risk change — moves OpenAI's index-freeze, Gemini's uuid tool-ids, Anthropic's signature/redacted deltas into per-provider `map_event`. Mitigated by moving behavior verbatim + golden `map_event` unit tests (TESTS.md) asserting byte-identical deltas.
- **Deleting `sampling_restricted` (ITEM-2) / `MODELS_REQUIRING_NON_STREAMING` (ITEM-3)**: both are private; grep confirms no external references. The gpt-5 dispatch in `openai.rs::stream_chat` (`:766-775`) must switch to `param_policy::openai_requires_non_streaming` — same model list, same behavior.
- **DB `ModelCapabilities` extension (ITEM-9)**: additive `Option` JSONB fields; repo load is `from_value(...).unwrap_or_default()` (`repository.rs:237-244`) so existing rows tolerate the new keys. No migration.

## Pattern conformance

- **ITEM-9/10 capability resolution** conforms to `available_files.rs:185-206` (DB `capabilities` preferred, catalog fallback) — the same layering already used for `supports_tool_use`.
- **ITEM-1 param_policy** mirrors `model_registry.rs` (pure, `OnceLock` catalog) + the pure `build_request_body` idiom (unit-tested, no I/O).
- **ITEM-7 sse.rs** extends the existing shared streaming infra in `providers/mod.rs`.
- **ITEM-6 typed errors** mirror `error.rs::from_anthropic_error` (sanitize both fields).
- **ITEM-14 UI** mirrors `LlmModelCapabilitiesSection` Switch pattern + the `vision`/`tools` Add/Edit wiring.
- **Tests** mirror the raw-`TcpStream` mock harness already in `anthropic.rs`'s test module (no new mock dep).

## Migration collisions

- **None.** No new migration is introduced. The DB capability extension (ITEM-9) reuses the existing schema-free `llm_models.capabilities` JSONB (migration `00000000000004`). `ls migrations/` is not consulted because nothing is added. The highest existing migration is unaffected.

## OpenAPI regen

- **Required (ITEM-15).** New `ChatRequest.model_caps` field and the new DB `ModelCapabilities` fields are schemars-exposed types that surface in the OpenAPI schema, so `just openapi-regen` must run and regenerate `openapi.json` + `api-client/types.ts` in **BOTH** `src-app/ui` and `src-app/desktop/ui`. The `emit_ts.rs` golden parity test (`types_ts_parity`) enforces they stay in lockstep. Generated files are excluded from the phase-6 coverage law but the regen itself is a required step. NOTE: verify whether `ChatRequest` is actually part of any exposed HTTP schema — it is an internal ai-providers type; if it is NOT surfaced in `openapi.json`, only the DB `ModelCapabilities` change drives the regen. Either way, run the regen and let the golden test confirm.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure module mirroring `model_registry.rs`; consults the catalog via `registry_lookup`; no caller impact until providers consume it.
- **ITEM-2** — verdict: PASS — replaces the private `sampling_restricted` gate with `resolve`; body-shape "one of temperature/top_p" stays local; existing anthropic `build_request_body` unit tests updated in TESTS.md.
- **ITEM-3** — verdict: CONCERN — must re-route the gpt-5 `requires_non_streaming` dispatch (`openai.rs:766-775`) through `param_policy` without changing which models trigger it; covered by a dedicated unit test (gpt-5 → disable_stream + max_completion_tokens).
- **ITEM-4** — verdict: CONCERN — Gemini currently sends a typed `GeminiRequest` via `.json(&req)`; serializing to `Value` first must produce byte-identical wire (same serde attrs). Guarded by a happy-path body snapshot test.
- **ITEM-5** — verdict: PASS — `finish_reason` field type unchanged; canonical value mapping is additive; downstream ripple handled in ITEM-11.
- **ITEM-6** — verdict: PASS — new `pub(crate)` parsers alongside the existing `from_anthropic_error`; no `ProviderError` variant change.
- **ITEM-7** — verdict: CONCERN — largest refactor; risk is behavior drift in per-provider delta mapping. Mitigated by verbatim move + golden `map_event` tests per provider. `non_streaming_to_stream` folded in as a response mode.
- **ITEM-8** — verdict: PASS — additive `Option` field; `Default`-derived; back-compat for all non-chat callers.
- **ITEM-9** — verdict: PASS — additive JSONB fields, no migration; tolerant load path already in place.
- **ITEM-10** — verdict: PASS — `thinking_config_for` + `apply_model_params` are private helpers in `streaming.rs`; re-removing forced 0.7 reverses the Step-0 revert deliberately (design requirement "no force-inject").
- **ITEM-11** — verdict: CONCERN — must audit every downstream finish-reason literal comparison (chat loop continue/stop; two test files) so canonical `tool_calls`/`stop`/`length` preserve semantics; the chat-internal `max_iterations`/`empty` markers are set by the loop (not provider) and are unaffected.
- **ITEM-12** — verdict: PASS — extends the single create path (`handlers/models.rs:144-178`), which already does one catalog lookup at create time; inference fills only when the request omits the flags.
- **ITEM-13** — verdict: PASS — curated catalog data addition; the prefix-tolerant `lookup` already covers dated SKUs; guarded by a registry unit test.
- **ITEM-14** — verdict: CONCERN — UI-touching ⇒ requires ≥1 e2e (phase 3) + `npm run check` + `gate:ui` (phase 8) in both workspaces; must add no new conditional render *state* (only more switches in an existing card) to avoid a `check:state-matrix` gate hit, or add the gallery cell.
- **ITEM-15** — verdict: PASS — mechanical `just openapi-regen`; generated files excluded from coverage; golden parity test enforces correctness.

**No BLOCKED verdicts.** CONCERNs (ITEM-3/4/7/11/14) are each pinned to a specific test in TESTS.md.

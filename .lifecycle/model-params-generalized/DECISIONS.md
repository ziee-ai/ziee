# DECISIONS — resolved up front

### DEC-1: Keep error-driven self-heal (send → 4xx → strip → retry)?
**Resolution:** No. Correctness comes entirely from the resolved request+response contract; a diverging model surfaces a clean typed error.
**Basis:** user

### DEC-2: Remove `known_models.json`?
**Resolution:** No — keep it as the curated *catalog layer* of the resolution chain. It is no longer required to be edited per model release.
**Basis:** user

### DEC-3: Capability precedence order?
**Resolution:** Per capability signal, highest wins — (1) DB model-row override via `ChatRequest.model_caps`, (2) curated catalog `known_models.json`, (3) provider model-family pattern, (4) conservative default; then request thinking/reasoning semantics applied as a restricting reconciliation (only removes/renames).
**Basis:** user (Option 1)

### DEC-4: Where do family patterns live, and what are the exact seeds?
**Resolution:** Pure predicates in `ai-providers/src/param_policy.rs`. OpenAI reasoning family = ids matching the `o`-series (`o1`/`o3`/`o4`/…) or `gpt-5` naming; Anthropic sampling-restricted = opus-class and Claude-5+ tiers; thinking-capable = the families currently flagged `supports_thinking` in the catalog (Anthropic opus/sonnet-4.6+/claude-5+, Gemini 2.5+, OpenAI o-series/gpt-5). Patterns match by prefix/family so new releases in a family are auto-covered.
**Basis:** codebase (seeds mirror the current catalog + `MODELS_REQUIRING_NON_STREAMING`)

### DEC-5: `finish_reason` — typed enum on the wire or canonical string?
**Resolution:** Keep `StreamChatChunk.finish_reason: Option<String>`; carry the canonical value produced via the internal `FinishReason` enum. Avoids rippling a type change across the serialized boundary and downstream string comparisons.
**Basis:** codebase (downstream compares literals; `chat/core/types/streaming.rs`)

### DEC-6: Canonical finish-reason vocabulary + per-provider mapping?
**Resolution:** Canonical set `stop | length | tool_calls | content_filter | refusal` (+ `Other(raw)` passthrough). Anthropic `end_turn|stop_sequence→stop`, `tool_use→tool_calls`, `max_tokens→length`, `refusal→refusal`; OpenAI `stop/length/tool_calls/content_filter` passthrough; Gemini `STOP→stop`, `MAX_TOKENS→length`, `SAFETY|RECITATION→content_filter`. The chat-loop's continue-on-`tool_calls` / stop semantics are preserved (Anthropic `tool_use` now canonicalizes to the `tool_calls` the loop already handles).
**Basis:** codebase + provider docs

### DEC-7: `max_tokens` default when unset?
**Resolution:** Keep both existing required-field defaults: `apply_model_params` chat-side default 8192, Anthropic `build_request_body` floor 1024. These are required-field defaults (Anthropic mandates `max_tokens`), not sampling defaults, so they are unaffected by the "no force-inject" rule.
**Basis:** codebase (existing behavior)

### DEC-8: `ModelParamContract` fields?
**Resolution:** `supports_sampling_params: Option<bool>`, `supports_thinking: Option<bool>`, `thinking_style: Option<String>`, `max_tokens_field: Option<MaxTokensField>` — all `Option` (None = "this source is silent, fall through").
**Basis:** codebase (subset of the catalog `ModelCapabilities` relevant to params)

### DEC-9: Is `ChatRequest.model_caps` exposed in OpenAPI?
**Resolution:** `ChatRequest` is an internal ai-providers type, not part of the HTTP schema; the OpenAPI regen is driven by the new DB `ModelCapabilities` fields (which ARE schema-exposed). Run `just openapi-regen` regardless and let the `types_ts_parity` golden test confirm lockstep.
**Basis:** codebase

### DEC-10: gpt-5 non-streaming model list source?
**Resolution:** `param_policy::openai_requires_non_streaming(model_id)`, seeded from the current `MODELS_REQUIRING_NON_STREAMING = ["gpt-5","gpt-5-mini"]`. Same models, same behavior, now in one policy place.
**Basis:** codebase

### DEC-11: OpenAI reasoning trigger — model family or request thinking?
**Resolution:** Both — reasoning when `openai_reasoning_family(id)` OR `req.thinking.mode != Disabled`. Matches the current `openai.rs` behavior (keys off `thinking`).
**Basis:** codebase

### DEC-12: UI — placement + new render state?
**Resolution:** Add the three capability controls to the existing `LlmModelCapabilitiesSection` card (two Switches + a small `thinking_style` select). No new conditional render state is introduced (they are additional fields in an existing loaded card), so no `check:state-matrix` gallery cell is required.
**Basis:** codebase

### DEC-13: `thinking_style` representation?
**Resolution:** `Option<String>` with the catalog vocabulary (`"adaptive"` | `"budget"`); the UI select offers those plus an empty (inherit) option.
**Basis:** codebase (matches registry `thinking_style`)

### DEC-14: Change both `ui` and `desktop/ui`?
**Resolution:** Yes — mirror the capability-section + drawer changes into `src-app/desktop/ui`; regen types in both.
**Basis:** codebase (workspace-mirroring + syncpack rule)

### DEC-15: Verification approach given no live keys?
**Resolution:** Deterministic in-crate mock/synthetic tests for request bodies + response/error parsing + the 4 scenarios; a UI e2e for the toggle. No live provider calls; never echo keys.
**Basis:** user

### DEC-16: Map `min_p`/`repeat_penalty`/`repeat_last_n` onto the wire?
**Resolution:** No — `ChatRequest` has no such fields; these remote-provider bodies don't accept them. Out of scope (unchanged from today).
**Basis:** codebase

### DEC-17: Removing forced 0.7 — update existing test expectations?
**Resolution:** Yes — update the `apply_model_params` inline test and `stub_chat_tier2_test.rs` to expect temperature omitted (the design's "no force-inject"; matches the reverted #122 intent). This is the deliberate re-application on top of the Step-0 revert.
**Basis:** codebase + user (design requirement)

### DEC-18: Non-chat callers that don't set `model_caps`?
**Resolution:** Acceptable — title/summarizer/memory/workflow/mcp-sampling pass `None`; `resolve` falls back to catalog+family. No behavior regression for them (they already relied on catalog/pattern-free sends).
**Basis:** codebase

# FIX_ROUND-2 — fixes from the round-2 re-audit; round-3 convergence

Round-2 blind re-audit (6 subagents, 12 angles) over the updated diff surfaced 7 new confirmed
defects. All were fixed (commit "audit round 2 — sanitize error type, tighten predicate, fix
regressed test"); adjudicated findings recorded in `LEDGER.jsonl`.

## Confirmed defects fixed
- **[regression HIGH]** existing integration test `empty_model_params_fall_back_to_defaults`
  (`stub_chat_tier2_test.rs`) still asserted the old forced 0.7 default → would fail. **Updated** to
  `empty_model_params_omit_temperature_and_default_max_tokens` (temperature omitted; `max_tokens`
  still 8192); refreshed the module doc.
- **[security MEDIUM]** `clean_http_error` sanitized the message but not the error `type`, and the
  pre-existing SSE mid-stream error path was unsanitized → **moved `sanitize_error_body` into
  `from_anthropic_error`** (sanitizes both `type` + `message`), covering every call site.
- **[tests-quality MEDIUM]** no end-to-end test of the `attempted_repair` guard on a persistent 400
  → **added** `stream_chat_retries_at_most_once_on_persistent_sampling_400` (exactly 2 requests, then
  a clean error).
- **[edge-cases/state LOW]** the new `UNSUPPORTED_HINTS` had over-broad tokens (`unexpected`,
  `not allowed`) → **tightened** to specific phrases + added negative predicate tests.
- **[api-contract LOW]** `clean_http_error` reclassified non-400 statuses by error type (e.g.
  529→RateLimit) → **scoped the parsed path to status 400**; other statuses keep `from_status_code`.
- **[naming LOW]** stale `top_k` doc in `models/chat.rs` → **updated**.
- **[naming LOW]** self-heal `warn!` lacked the sibling `"Provider:"` prefix → **`"Anthropic: …"`**.

## Adjudicated (documented in LEDGER, no code change)
- `spawn_mock` `.unwrap()`s — test-only; the 15s timeout is the intended fail-fast backstop.
- `resp.text()` reads the full body before truncation — pre-existing + codebase-wide across all
  providers; a response-size cap is separate cross-cutting hardening.
- hand-rolled TcpListener mock is a new idiom — no existing in-crate mock idiom; self-contained.
- `chat_request` vs `req()` test-helper duplication — acceptable across module boundaries.
- clean-error type-vs-status mapping — resolved by the status-400 scoping above.

## Re-audit (round 3)
A fresh 4-subagent blind round (all 12 angles) over the final diff found **zero** new confirmed
defects — every angle returned clean, explicitly verifying the round-1 + round-2 fixes hold
(fail-closed predicate, full sanitization coverage, bounded retry, updated tests, no stale docs).
Convergence reached.

**New confirmed findings:** 0

# FIX_ROUND-1 — fixes from the round-1 blind audit; round-2 re-audit result

Round-1 blind audit = 6 fresh subagents (12 angles) over `git diff khoi...HEAD`. All CONFIRMED
defects were fixed (commit "harden self-heal per audit"); adjudicated findings recorded in
`LEDGER.jsonl`.

## Confirmed defects fixed
- **[edge-cases MEDIUM]** `is_sampling_param_400` matched any 400 mentioning a sampling word, so an
  invalid-VALUE 400 (`temperature=5.0`) was stripped+retried, silently answering at the provider
  default → **narrowed to `is_unsupported_sampling_error`, fail-closed** (requires an
  unsupported-param indicator; never repairs value errors).
- **[security MEDIUM]** the surfaced Anthropic error message wasn't sanitized (bypassed
  `sanitize_error_body`) → **sanitized** (bound + newline-collapse).
- **[security LOW]** self-heal `warn!` logged the raw operator `model` → **sanitized** it.
- **[concurrency/tests MEDIUM]** the loopback test had no timeout (a non-retry regression would hang
  CI) → **wrapped in `tokio::time::timeout`**.
- **[patterns LOW]** helpers carried a redundant `anthropic_` prefix → **renamed**
  (`clean_http_error`, `parse_error_envelope`).
- **[patterns/naming LOW]** stale/ misleading docs (`parse` "mirrors SSE", `sampling_restricted`,
  registry field doc) → **refreshed** to include the Claude 5 family.
- **[perf LOW]** the error body was JSON-parsed twice → **parse once, reuse**.
- **[tests-quality LOW]** untested fallback / no negative retry path → **added** an invalid-value
  no-retry loopback test + a sanitization test; removed the vague `"sampling"` keyword branch.

## Adjudicated (documented in LEDGER, no code change)
- strip-all-three-sampling-keys — by-design (Anthropic sampling is all-or-nothing per model).
- ITEM-4's 0.7-default removal is provider-wide (OpenAI/Gemini) — **intended per task item #4**;
  documented for human confirmation.
- `claude-sonnet-5` `supports_sampling_params:false` — confirmed via Anthropic docs (Claude 5 rejects
  sampling params).
- self-heal not wired into embeddings/upload — those endpoints send no sampling params.

## Re-audit (round 2)
A fresh 6-subagent blind round over the updated diff surfaced NEW confirmed findings (a regressed
integration test, an unsanitized error `type` + SSE path, an over-broad predicate hint, a missing
persistent-400 guard test, a non-400 reclassification, and stale docs). Loop not yet converged.

**New confirmed findings:** 7

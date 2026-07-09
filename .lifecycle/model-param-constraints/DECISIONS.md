# DECISIONS — model-param-constraints

Every human/product input resolved up front so implementation runs nonstop.

### DEC-1: Which Claude 5 SKUs to add to `known_models.json`?
**Resolution:** Add `claude-sonnet-5` only. Fable 5 / Mythos 5 are not added.
**Basis:** user — chose "Sonnet 5 only" at plan approval. Fable/Mythos are covered at runtime by the
self-heal (ITEM-3) if a user manually adds them.

### DEC-2: Persist a learned 400 sampling restriction onto the DB model `capabilities`?
**Resolution:** Deferred — not in this PR.
**Basis:** user — chose "defer" at plan approval. The DB-side `ModelCapabilities` struct carries no
thinking/sampling flags, so persistence would widen scope (new fields + write path) for marginal
benefit over the registry stopgap + in-memory self-heal.

### DEC-3: Where does the self-heal retry live — provider layer or the chat service?
**Resolution:** In the provider layer, inside `anthropic.rs::stream_chat`.
**Basis:** codebase — `stream_chat` is the single completion path (no non-streaming `chat` method
exists), so placing the retry there benefits every caller (chat + workflow) and keeps it
unit-testable without the chat stack.

### DEC-4: When thinking is active, omit the sampling block entirely or coerce `temperature` to 1?
**Resolution:** Omit `temperature`/`top_p`/`top_k` entirely.
**Basis:** convention — Anthropic defaults `temperature` to 1 when the sampling block is absent, so
omission is the simplest robust behavior and matches the plan's recommended approach; it also avoids
inventing a value the operator did not configure.

### DEC-5: Add a new `ProviderError` variant for the self-heal path, or reuse an existing one?
**Resolution:** Reuse `ProviderError::from_anthropic_error` (→ `InvalidRequest(message)`); no new variant.
**Basis:** codebase — `from_anthropic_error` already maps `invalid_request_error` to a clean
`InvalidRequest(message)`, and the SSE-error path already parses the same envelope shape.

### DEC-6: Keep the `max_tokens` 8192 fallback in `apply_model_params` when dropping the temperature default?
**Resolution:** Keep the `max_tokens` fallback; only the `temperature` fallback is removed.
**Basis:** convention — Anthropic requires `max_tokens` on every request, so a sane default must
remain; `temperature` is optional and provider-defaulted, so its forced default is what to drop.

### DEC-7: How many retries on a repairable 400?
**Resolution:** Retry exactly once, then surface the clean error.
**Basis:** user — the task specifies "retry once"; a single retry covers the strip-and-resend case
without risking a loop.

# DRIFT-1 — implementation vs plan

Implementation complete for all ITEMs; `cargo check -p ziee` (exit 0, only a pre-existing unrelated
dead-code warning in `mcp/repository.rs`) and `cargo test -p ai-providers --lib` (77 pass) +
`cargo test --lib -p ziee apply_model_params` (pass). Auditing each ITEM against PLAN.md:

- **DRIFT-1.1** — verdict: none — ITEM-1 implemented exactly as planned: `thinking_active` computed from `request.thinking.mode` (Adaptive|Enabled) and folded into `sampling_ok`; sampling block omitted when thinking active. Verified by `adaptive_thinking_omits_sampling_on_allowed_model` / `enabled_thinking_omits_sampling_keeps_budget` / `no_thinking_allowed_model_keeps_temperature`.
- **DRIFT-1.2** — verdict: none — ITEM-2 implemented as planned: `claude-sonnet-5` added mirroring `claude-opus-4-8` (`supports_sampling_params:false`, adaptive thinking). Verified by `sonnet_5_thinking_adaptive_and_sampling_restricted` / `dated_sonnet_5_resolves_to_base_and_stays_restricted` / `sonnet_5_omits_sampling_via_registry`.
- **DRIFT-1.3** — verdict: none — ITEM-3 implemented as planned in `stream_chat`: single guarded retry (`attempted_repair` flag) on a sampling-param 400; clean error via `anthropic_http_error`→`from_anthropic_error`. Added one extra small pure helper `parse_anthropic_error` (the plan called for parsing the envelope; this factors it) — an in-scope refinement, not a divergence. Verified by the three self-heal unit tests + the `stream_chat_self_heals_sampling_400_and_retries_once` loopback integration test.
- **DRIFT-1.4** — verdict: none — ITEM-4 implemented as planned: `req.temperature = p.temperature` (0.7 default removed), `max_tokens` default kept, doc comment updated, `apply_model_params_maps_and_defaults` test updated (unset ⇒ `None`). Verified green.
- **DRIFT-1.5** — verdict: none — No unplanned files touched; no migration added; no OpenAPI/`types.ts` regen needed (confirmed: only `anthropic.rs`, `model_registry.rs`, `known_models.json`, `streaming.rs` changed). Matches PLAN "Files to touch".

**Unresolved drifts:** 0

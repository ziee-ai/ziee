# PLAN_AUDIT — audited against the codebase

## Breakage risk

- **ITEM-1** — Introducing `pub const ANTHROPIC_VERSION = "2023-06-01"` and
  replacing the three existing `.header("anthropic-version", "2023-06-01")`
  literals is a **value-preserving refactor**: the emitted header bytes are
  identical, so the chat/messages, file-upload, and file-delete requests are
  unchanged at runtime. The new header on the discovery probe is **additive** —
  it only makes a previously-400 request succeed. No caller reads the literal by
  string. Risk: negligible.
- **ITEM-2** — Adding a `display_name` fallback in `parse_one_live_model` is a
  read of a new optional JSON field with the SAME precedence semantics
  (`name` still wins; `display_name` fills only when `name` is absent, e.g. for
  Anthropic which never sends `name`). OpenRouter/OpenAI/Gemini bodies carry no
  `display_name`, so their parsed output is byte-identical. The existing 4 parser
  unit tests continue to pass unchanged.
- **ITEM-3** — Verify-only + appended e2e test. No product-source change, so no
  runtime behavior change. The new spec uses a fresh provider (unique name), so
  it does not disturb existing specs.

**Coordination:** ITEM-1 touches only header-string lines in `anthropic.rs`
(`:680,:959,:1020`), far from the chat request-**param** body builders owned by
the `model-param-fix` worker; distinct lines → negligible merge-conflict risk.

## Pattern conformance

- **ITEM-1** — const placement + `pub use` re-export mirror the existing
  `ai-providers/src/lib.rs` export block and the module-top items in
  `anthropic.rs`; the discovery `match` arm mirrors the existing arm shape at
  `discover.rs:256-262` and references the crate exactly as the file already does
  (`ai_providers::registry_*` → `ai_providers::ANTHROPIC_VERSION`).
- **ITEM-2** — the `.or_else(...)` fallback mirrors the existing option-chaining
  idiom already used throughout `discover.rs` (e.g. the `display_name`/
  `context_length` merges at `:160-186`).
- **ITEM-3** — TEST-2 mirrors `discover_enriches_openrouter_models_and_gates_permission`
  (wiremock + `LLM_DISCOVER_ALLOW_LOOPBACK` seam); TEST-3 mirrors the two existing
  cases in `remote-model-picker.spec.ts`. Conforms to
  [[feedback_match_existing_patterns]].

## Migration collisions

None. No SQL migration is added or touched; no `migrations/` change; no
`cargo clean` / build-db reset needed. No collision with any concurrent worker's
migration numbering.

## OpenAPI regen

Not required. The response contract (`DiscoverModelsResponse` /
`DiscoveredModel`) is unchanged — no new/renamed/removed fields, no new endpoint.
`parse_live_models`/`fetch_live_models` are internal (`pub(crate)`) and not part
of the emitted schema. Therefore `openapi.json` + `api-client/types.ts` are NOT
regenerated, and the diff stays backend-only (no generated-FE artifact churn).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — value-preserving const extraction + additive header; mirrors existing re-export and match-arm patterns; no migration/OpenAPI impact.
- **ITEM-2** — verdict: PASS — additive optional-field fallback; preserves existing parser outputs; existing unit tests unaffected.
- **ITEM-3** — verdict: PASS — verify-only + appended e2e mirroring existing picker specs; no product-source change, no OpenAPI/migration impact.

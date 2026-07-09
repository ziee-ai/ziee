# DECISIONS — resolved up-front (every question answered before implementation)

### DEC-1: What `anthropic-version` value should the discovery probe send?
**Resolution:** `2023-06-01` — the exact value the main Anthropic chat/file
client already sends at `anthropic.rs:680,959,1020`. Extract it once as
`pub const ANTHROPIC_VERSION` in `ai-providers` and reference it from all four
sites (3 existing + the discovery probe).
**Basis:** codebase — this is the version pinned across the existing Anthropic
integration; a divergent copy is the anti-pattern the task calls out.

### DEC-2: Where does the shared constant live, and how is it exported?
**Resolution:** Define `pub const ANTHROPIC_VERSION: &str = "2023-06-01";` at the
top of `ai-providers/src/providers/anthropic.rs`, and re-export it from
`ai-providers/src/lib.rs` as `pub use providers::anthropic::ANTHROPIC_VERSION;`
(the `providers` module already exposes the anthropic submodule) so `discover.rs`
can use `ai_providers::ANTHROPIC_VERSION`, matching how it already calls
`ai_providers::registry_*`.
**Basis:** convention — mirrors the existing re-export block in `lib.rs`.

### DEC-3: Should the parser infer capabilities (vision/tools/context) for
Anthropic models from `/v1/models`?
**Resolution:** No. Anthropic's `/v1/models` returns only
`{type,id,display_name,created_at}` — no capability fields. Parse `id` (already
handled) + `display_name` (ITEM-2) only; the curated catalog remains the source
of truth for capabilities, consistent with the handler's existing merge logic
(`discover.rs:150-195`).
**Basis:** codebase — the handler already treats the catalog as authoritative for
capability values and the live call as an ID/label augment.

### DEC-4: Should the frontend "Select a model" component be changed?
**Resolution:** No source change. The picker is already non-blocking (options
derive purely from `models[]`; the note renders as a separate `Alert tone="info"`
that never disables the `Combobox`). The reported symptom is the failed live
discovery (fixed by ITEM-1), not a UI block. Lock the graceful-degradation
behavior with the TEST-3 e2e regression guard instead of editing the component.
**Basis:** codebase — confirmed by reading `AddRemoteLlmModelDrawer.tsx:79-221`
and `LlmProvider.store.ts:337-362`, corroborated by an Explore-agent trace.

### DEC-5: How does TEST-2 prove the header is actually sent (not just that
discovery returns models)?
**Resolution:** Mount the wiremock `/models` stub behind a
`wiremock::matchers::header("anthropic-version", "2023-06-01")` matcher. If the
header is absent the mock returns the default 404 → the handler emits a
fallback note and the model is absent; the test asserts the model IS present with
`source: "discovery"` and no fallback note, which can only happen when the header
was sent. This is a positive proof that also fails closed if the header regresses.
**Basis:** codebase — wiremock `header` matcher is already a dependency used by
the existing discovery test's `method`/`path` matchers.

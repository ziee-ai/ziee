# PLAN — Anthropic model-discovery 400 fix + Select-a-model verification

## Problem

Adding a model to an **Anthropic** provider via the "Select a model" box shows
`live /v1/models call failed; falling back to catalog only: HTTP 400 Bad Request`.
Root cause: the discovery probe `fetch_live_models`
(`src-app/server/src/modules/llm_provider/handlers/discover.rs:256-262`) sends the
Anthropic `GET /v1/models` request with only `x-api-key`; Anthropic requires the
`anthropic-version` header on every request (the chat client already sends
`2023-06-01` at `ai-providers/src/providers/anthropic.rs:680,959,1020`). Missing
it → 400 → fallback to the 4-entry static catalog + an alarming note.

The frontend picker is already non-blocking (options derive purely from
`models[]`; the note is a separate `Alert tone="info"`), so the fix is backend;
the frontend is confirm-only + an e2e regression guard.

## Items

- **ITEM-1**: Add the `anthropic-version` header to the Anthropic branch of the discovery probe in `discover.rs::fetch_live_models`, sourcing the value from a new shared `pub const ANTHROPIC_VERSION` in `ai-providers` (replacing the three hardcoded literals in `anthropic.rs`) so there is no divergent copy of the version string.
- **ITEM-2**: Extend `discover.rs::parse_one_live_model` to read a model's display name from Anthropic's `display_name` field (fallback after the existing `name` field), so discovered Claude models carry a human label. Model IDs already populate via the existing `data[].id` path.
- **ITEM-3**: Verify (no source change) that the Add-Remote-Model picker still lets the user select a model from the catalog fallback when live discovery fails, and that the fallback note is shown non-blockingly; lock this with an e2e regression test.

## Files to touch

- `src-app/server/ai-providers/src/providers/anthropic.rs` — add `pub const ANTHROPIC_VERSION`, replace 3 literals.
- `src-app/server/ai-providers/src/lib.rs` — re-export `ANTHROPIC_VERSION`.
- `src-app/server/src/modules/llm_provider/handlers/discover.rs` — add header (ITEM-1); parse `display_name` (ITEM-2); add a `parse_live_models` unit test (TEST-1).
- `src-app/server/tests/llm_provider/discover_models_test.rs` — new Anthropic integration test (TEST-2).
- `src-app/ui/tests/e2e/llm/remote-model-picker.spec.ts` — append the Anthropic fallback-non-blocking regression test (TEST-3).

## Patterns to follow

- **Shared constant** — mirror the existing `pub use providers::{...}` re-export style in `ai-providers/src/lib.rs`; keep the const at the top of `anthropic.rs` next to the provider struct, doc-commented like the surrounding items.
- **Discovery header branch** — mirror the existing `match provider_type` arm shape in `discover.rs:256-262`; reference the const via `ai_providers::ANTHROPIC_VERSION` (the crate is already used as `ai_providers::registry_*`).
- **`parse_live_models` unit test** — mirror the existing `#[cfg(test)] mod tests` cases in `discover.rs:360-438` (`serde_json::json!` body → assert fields).
- **Integration test** — mirror `discover_models_test.rs::discover_enriches_openrouter_models_and_gates_permission` (wiremock `MockServer` at 127.0.0.1, `LLM_DISCOVER_ALLOW_LOOPBACK=1` via `TestServerOptions.extra_env`, `create_provider` helper); add a `wiremock::matchers::header("anthropic-version", "2023-06-01")` matcher so the mock only responds when the header is present.
- **E2e** — mirror `remote-model-picker.spec.ts` (`createProviderViaAPI(apiURL, token, name, 'anthropic')`, `assignProviderToAdministratorsGroup`, `goToProviderDetail`, `byTestId('llm-remote-model-picker')`); assert the picker is enabled and lists a catalog Claude model while the info note (`llm-remote-discover-notes`) is present.

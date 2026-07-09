# TESTS — explicit enumeration (every ITEM ↔ ≥1 TEST)

- **TEST-1** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/llm_provider/handlers/discover.rs` — asserts: `parse_live_models("anthropic", body)` on an Anthropic-shaped body `{"data":[{"type":"model","id":"claude-opus-4-8","display_name":"Claude Opus 4"}]}` returns one `LiveModel` with `id == "claude-opus-4-8"` and `display_name == Some("Claude Opus 4")`; and a body whose item has only `id` (no `name`/`display_name`) yields `display_name == None`.
- **TEST-2** (tier: integration) [covers: ITEM-1] file: `src-app/server/tests/llm_provider/discover_models_test.rs` — asserts: `GET /llm-providers/{id}/discover-models` for an `anthropic` provider issues the upstream `/models` request carrying BOTH `x-api-key` and `anthropic-version: 2023-06-01` (enforced by a wiremock `header` matcher that returns 200 only when the header is present), and the mocked Claude model appears in the response `models[]` with `source == "discovery"` and no live-fallback note.
- **TEST-3** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/llm/remote-model-picker.spec.ts` — asserts: for an `anthropic` provider with an invalid key (live `/v1/models` 400 → catalog fallback + an info note), opening the Add-Remote-Model drawer leaves the picker **enabled** and populated with a catalog Claude model (`claude-opus-4-8`), the discovery note Alert (`llm-remote-discover-notes`) is visible, and the user can still select the model — proving a fallback note never disables/empties the selector.

## Coverage map
- ITEM-1 → TEST-2
- ITEM-2 → TEST-1
- ITEM-3 → TEST-3 (e2e, satisfies the UI-touch e2e requirement)

# TESTS — ask-user-decision-ux

Every ITEM is covered by ≥1 TEST. Frontend items carry an e2e spec. Mock only
the external boundary (the SSE chat stream / the `/respond` POST) — the renderer,
zod validation, wizard state, and store roundtrip run for real.

## Backend

- **TEST-1** (tier: unit)        [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/helpers.rs` — asserts: `stamp_ask_user_marker` adds `"x-ziee-askuser": true` to an object schema, is idempotent, and returns non-object schemas unchanged (no panic).
- **TEST-2** (tier: unit)        [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/helpers.rs` — asserts: the size/injection guard still trips FIRST — an oversized raw schema returns the "too large" error result and is NEVER stamped (guard precedes stamp), i.e. the existing `MAX_STRUCTURED_CONTENT_BYTES` reject is preserved.
- **TEST-3** (tier: integration) [covers: ITEM-1, ITEM-9] file: `src-app/server/tests/mcp/elicitation_mcp_test.rs` — asserts: driving `ask_user` end-to-end (register → SSE `mcpElicitationRequired` → `/respond` accept) surfaces a `requested_schema` carrying `x-ziee-askuser:true`, and the accepted content round-trips back as the flat `{prop: value}` tool result (envelope unchanged).
- **TEST-4** (tier: unit)        [covers: ITEM-2] file: `src-app/server/src/modules/elicitation_mcp/tools.rs` — asserts: the descriptor still exposes exactly one `ask_user` tool with `required = [message, schema]` and unchanged `inputSchema` property types, and the description text documents the rich conventions (`enumDescriptions`, `x-ziee-recommended`, `x-ziee-allow-other`).

## Frontend — unit

- **TEST-5** (tier: unit)        [covers: ITEM-3] file: `src-app/ui/src/modules/mcp/chat-extension/components/elicitationOptions.test.ts` — asserts: `getRichOptions` maps `enum`+`enumNames`+`enumDescriptions`+`enumPreviews` (and the `oneOf/anyOf` titled form with `description`/`preview`/`recommended`) into `{value,label,description,preview,recommended}[]`, for single and multi (`items`) shapes.
- **TEST-6** (tier: unit)        [covers: ITEM-3] file: `src-app/ui/src/modules/mcp/chat-extension/components/elicitationOptions.test.ts` — asserts: `buildFormSchema` still enforces required/format/pattern/min-max exactly as before (email/uri/pattern/number bounds/multiselect min-max), i.e. the extraction preserved validation.
- **TEST-7** (tier: unit)        [covers: ITEM-5] file: `src-app/ui/src/modules/mcp/chat-extension/components/elicitationOptions.test.ts` — asserts: `orderRecommendedFirst` moves the `x-ziee-recommended` value (and a `oneOf` `recommended:true` entry) to index 0 and flags it, leaving other order stable; no recommended → order unchanged.
- **TEST-8** (tier: unit)        [covers: ITEM-6] file: `src-app/ui/src/modules/mcp/chat-extension/components/elicitationOptions.test.ts` — asserts: choice fields get an Other affordance by default and opt out on `x-ziee-allow-other:false` (`isChoiceField` + the allow-other resolver), and `OTHER_SENTINEL` is distinct from any realistic option value.

## Frontend — e2e

- **TEST-9** (tier: e2e)         [covers: ITEM-4, ITEM-9] file: `src-app/ui/tests/e2e/chat/ask-user-decision-ux.spec.ts` — asserts: a stamped single-select ask_user schema renders radio-CARDS (each option shows its `enumDescriptions` text, not a dropdown), selecting a card + Submit POSTs `accept` with the chosen value; an UN-stamped generic schema still renders the legacy control (parity).
- **TEST-10** (tier: e2e)        [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/ask-user-decision-ux.spec.ts` — asserts: the `x-ziee-recommended` option is rendered FIRST and shows a "Recommended" badge.
- **TEST-11** (tier: e2e)        [covers: ITEM-6] file: `src-app/ui/tests/e2e/chat/ask-user-decision-ux.spec.ts` — asserts: the "Other…" card is present, clicking it reveals a text input, typing a custom value + Submit POSTs `accept` with the typed free-text as the field answer.
- **TEST-12** (tier: e2e)        [covers: ITEM-7] file: `src-app/ui/tests/e2e/chat/ask-user-decision-ux.spec.ts` — asserts: a 2-property stamped schema renders a wizard (step "1 of 2", Back disabled on step 1); Next is blocked until the required step-1 choice is made, Back returns preserving the choice, and a single final Submit POSTs `accept` with BOTH answers in one flat content object.
- **TEST-13** (tier: e2e)        [covers: ITEM-7] file: `src-app/ui/tests/e2e/chat/ask-user-decision-ux.spec.ts` — asserts: Decline on a wizard step POSTs `decline` and shows the declined card (decline/cancel preserved on the rich path).
- **TEST-14** (tier: e2e)        [covers: ITEM-8] file: `src-app/ui/tests/e2e/chat/ask-user-decision-ux.spec.ts` — asserts: an option carrying a preview renders its monospace preview block (`-opt-<value>-preview`); options without a preview render none.
- **TEST-15** (tier: e2e)        [covers: ITEM-2] file: `src-app/ui/tests/e2e/chat/ask-user-elicitation.spec.ts` — asserts: the existing assistant-labelled ask_user flow still round-trips a choice (updated for the stamped/card rendering) — the back-compat headline case stays green under the new renderer.
- **TEST-16** (tier: e2e)        [covers: ITEM-10] file: `src-app/ui/tests/e2e/visual/` (gallery) — asserts: the new rich ask_user gallery deep-state renders with zero runtime HIGH findings (console/contrast) under `npm run gate:ui`; covered via the gallery state cell + `check:state-matrix` inside `npm run check`.

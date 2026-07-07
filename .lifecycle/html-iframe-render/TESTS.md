# TESTS — enumerated up front (every ITEM ↔ ≥1 TEST)

The `src-app/ui` workspace has **no** unit-test runner (no vitest — verified: no
`test`/`vitest` npm script, no `*.test.ts(x)`, `vite.config.ts` only). Frontend
verification here is Playwright **e2e** (real render path, mocked SSE — no LLM)
plus the deterministic **gallery affordance detector** (`integration`-tier: it
drives the real `ConversationPage` through the gallery and asserts DOM presence).
Per [[feedback_no_cosmetic_tests]] the security proof exercises the REAL iframe
in a REAL browser (reads the actual `sandbox` attr + proves cross-origin
isolation), not a stubbed string check.

E2E specs live in `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts`,
seeding an assistant message via `mockChatStream`/`mockGetMessages`
(mirroring `markdown-rendering.spec.ts`).

## Tests

- **TEST-1** (tier: e2e) [covers: ITEM-1, ITEM-6] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: an assistant ```` ```html ```` fence renders an `[data-testid="html-block"]` container defaulting to the CODE view (source text visible, `[data-testid="html-block-toggle"]` present, NO `<iframe>` in the DOM yet).
- **TEST-2** (tier: e2e) [covers: ITEM-1, ITEM-3] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: clicking `html-block-toggle-opt-preview` mounts an `<iframe>` whose `sandbox` attribute is exactly `allow-scripts` (contains `allow-scripts`, does NOT contain `allow-same-origin`/`allow-top-navigation`/`allow-popups`/`allow-forms`) and whose `srcdoc` is populated.
- **TEST-3** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: the preview iframe's `srcdoc` contains the injected CSP `<meta http-equiv="Content-Security-Policy">` with `default-src 'none'` (blocks external network), and the iframe carries `referrerpolicy="no-referrer"`.
- **TEST-4** (tier: e2e) [covers: ITEM-2, ITEM-3] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: SECURITY — an HTML payload whose inline `<script>` tries `window.parent.__HTML_PWNED = true` (and `top.location=...`) RUNS inside the sandbox but CANNOT reach the parent: after opening Preview, the parent-page flag stays `false`/undefined and the top URL is unchanged (null-origin isolation holds).
- **TEST-5** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: while the ```` ```html ```` fence is still streaming (incomplete), the block stays in CODE view with the Preview option disabled and NO `<iframe>` mounts, and no `pageerror` fires during the stream.
- **TEST-6** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: the `html` language label is shown and clicking `[data-testid="html-block-copy-btn"]` writes the exact HTML source to the clipboard (read back via `navigator.clipboard.readText`).
- **TEST-7** (tier: integration) [covers: ITEM-7, ITEM-8] file: `src-app/ui/scripts/affordance-audit.mjs` — asserts: running the affordance-audit against the booted gallery finds ≥1 `[data-testid="html-block"]` container in `deep-chat-rendering-showcase` and the guarding `html-render` rule reports 0 gating misses (the `html-block-toggle` control is present under every html-block container).
- **TEST-8** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/chat/html-iframe-render.spec.ts` — asserts: toggling Preview→Code returns to the source view (the `<iframe>` unmounts / source text visible again), proving the toggle is bidirectional and per-block-stateful.

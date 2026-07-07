# TEST_RESULTS — Mermaid code⇄render toggle

All Phase-3 tests executed. Frontend-only change (workspace `ui`); no backend /
cargo tests in scope.

## Frontend gate

- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors +
  lint:settings-field + lint:adjacent-inline + lint:icon-action +
  check:kit-manifest + check:testid-registry + check:design-spec +
  check:gallery-coverage + check:gallery-crawl + gallery:check-fixtures +
  check:state-matrix.
- tsc --noEmit (desktop/ui, compiles the shared component via `@/*`→`../../ui/src`): PASS.

## e2e (`tests/e2e/visual/mermaid-toggle.spec.ts`, Playwright vs the gallery — 6 passed / 17.6s)

Interactive tests run against the isolated `deep-chat-rendering-showcase` deep-state
(the REAL ConversationPage chat path, no gallery overlay backdrops); state-visibility
tests run against the `mermaid-block` component story.

- **TEST-1**: PASS — mermaid block renders a real `<svg>` by default via the production chat path.
- **TEST-2**: PASS — toggle defaults to Diagram, flips to Source (raw `graph TD`, svg gone) and back.
- **TEST-3**: PASS — invalid diagram → inline error (no svg, no page error); streaming → deferred placeholder.
- **TEST-4**: PASS — copy-source writes the exact mermaid source to the clipboard.
- **TEST-5**: PASS — download-svg disabled while streaming, enabled on a rendered diagram, and the downloaded file is a real `.svg` whose bytes contain `<svg`.
- **TEST-6**: PASS — all four story cases (render/source/error/streaming) reach their terminal state with no uncaught page error.

## Detector guard (affordance-audit)

- **TEST-7**: PASS — `node scripts/affordance-audit.mjs` exits 0 with "Allowlisted gaps: 0" and "Gating misses (HIGH): 0": the `mermaid-toggle` rule is now GUARDED (de-allowlisted) and satisfied — the deep-chat mermaid block contains `[data-testid="mermaid-source-toggle"]`.

## Notes

- No `#[ignore]` / skips. The one env caveat (clipboard-read) is satisfied by the
  visual project's Desktop-Chrome runner, where clipboard permissions are granted.

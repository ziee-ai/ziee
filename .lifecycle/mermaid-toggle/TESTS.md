# TESTS — Mermaid code⇄render toggle

Every PLAN ITEM is covered by ≥1 test.

**Tier note (why all e2e):** the main `src-app/ui` workspace has NO unit-test
runner — vitest exists only in `src-app/desktop/ui`, and `ui`'s established test
strategy is **Playwright e2e against the component gallery** (the stable,
backend-free surface the whole UI-testing system runs against) plus the
`npm run check` static gates (affordance-audit / runtime-health / visual). So the
honest tier for this UI feature is e2e-against-the-gallery, not a unit runner this
workspace doesn't have ([[feedback_comprehensive_tests]] — mirror the codebase's
existing tier pattern; see DEC-7). No cosmetic tests: the mermaid render path runs
for real (real `mermaid` package + real Streamdown in the browser); only the app
shell is the gallery mock ([[feedback_no_cosmetic_tests]]).

## e2e — behavioral (`tests/e2e/visual/mermaid-toggle.spec.ts`, Playwright vs the gallery)

- **TEST-1** (tier: e2e) [covers: ITEM-1, ITEM-6, ITEM-7] file: `src-app/ui/tests/e2e/visual/mermaid-toggle.spec.ts` — asserts: on the mermaid gallery story, the render-mode case shows a `[data-streamdown="mermaid-block"]` card whose body contains a real rendered `<svg>` by default (no toggle click) — proving the custom renderer produces the card AND a valid diagram renders live.
- **TEST-2** (tier: e2e) [covers: ITEM-2] file: `src-app/ui/tests/e2e/visual/mermaid-toggle.spec.ts` — asserts: `[data-testid="mermaid-source-toggle"]` defaults to Diagram (`-opt-render` has `data-state="on"`); clicking the Source segment hides the `<svg>` and reveals the raw mermaid source in a `<pre>`; clicking Diagram returns to the `<svg>` — BOTH modes exercised live.
- **TEST-3** (tier: e2e) [covers: ITEM-3] file: `src-app/ui/tests/e2e/visual/mermaid-toggle.spec.ts` — asserts: the invalid-diagram gallery case renders the inline error state (an error message, no `<svg>`) WITHOUT blanking the surface (no ErrorBoundary fallback / no page error); and the streaming (`isIncomplete`) case shows the deferred placeholder, not an errored render.
- **TEST-4** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/mermaid-toggle.spec.ts` — asserts: with clipboard permission granted, clicking `[data-testid="mermaid-copy-source-btn"]` results in `navigator.clipboard.readText()` equal to the exact mermaid source and shows the success toast.
- **TEST-5** (tier: e2e) [covers: ITEM-5] file: `src-app/ui/tests/e2e/visual/mermaid-toggle.spec.ts` — asserts: `[data-testid="mermaid-download-svg-btn"]` is disabled on the streaming case (no SVG yet) and enabled on the rendered case; clicking it on the rendered case fires a download whose suggested filename ends `.svg`.
- **TEST-6** (tier: e2e) [covers: ITEM-7] file: `src-app/ui/tests/e2e/visual/mermaid-toggle.spec.ts` — asserts: the mermaid story renders all four gallery cases each reaching its expected terminal state (render→`<svg>`, source→`<pre>` source view, error→inline error, streaming→deferred placeholder) with NO uncaught page error — the both-modes state-matrix exercise.

## e2e — detector guard (affordance-audit)

- **TEST-7** (tier: e2e) [covers: ITEM-8] file: `src-app/ui/scripts/affordance-audit.mjs` — asserts: run against the running gallery, the `mermaid-toggle` rule reports as a GUARDED (non-allowlisted) PASS — i.e. every `[data-streamdown="mermaid-block"]` contains `[data-testid="mermaid-source-toggle"]`, and the allowlist no longer excuses it (`affordance-audit.mjs` exits 0 with the rule un-allowlisted).

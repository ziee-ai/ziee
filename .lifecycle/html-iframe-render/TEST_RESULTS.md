# TEST_RESULTS — Phase 8 gated run

Scope: the frontend feature only ([[feedback_test_scope]]). No backend change,
so no cargo suite. Frontend gate = `npm run check (ui)` + the enumerated
Playwright e2e specs + the affordance-audit integration check.

## Frontend gate

- **npm run check (ui): PASS** — `tsc && lint:guardrails && lint:colors &&
  lint:settings-field && lint:adjacent-inline && lint:icon-action &&
  check:kit-manifest && check:testid-registry && check:design-spec &&
  check:gallery-coverage && check:gallery-crawl && gallery:check-fixtures &&
  check:state-matrix` all green.

## Commands

```bash
# e2e (7 tests, chromium, --workers=1; mock SSE — no LLM)
cd src-app/ui && npx playwright test tests/e2e/chat/html-iframe-render.spec.ts \
    --project=chromium --workers=1
#  → 7 passed (1.6m)

# TEST-7 integration — affordance-audit against the booted gallery
node scripts/affordance-audit.mjs --themes=light
#  → 14 deep-chat states audited · Gating misses (HIGH): 0
#  (html-block container=1 + html-block-toggle=1 confirmed present in
#   deep-chat-rendering-showcase — non-vacuous; both modes render: code default +
#   the html-preview interaction shows the sandbox=allow-scripts iframe.)
```

## Per-test results

- **TEST-1**: PASS — html fence defaults to CODE view (source shown, toggle present, no iframe).
- **TEST-2**: PASS — Preview mounts an iframe with `sandbox="allow-scripts"` (no allow-same-origin/top-nav/popups/forms); srcdoc populated.
- **TEST-3**: PASS — external `<img>` is blocked BY the CSP as observed inside the null-origin frame (IMG_BLOCKED, with a fulfilling route so a would-be-load is distinguishable); srcdoc carries `default-src 'none'`; referrerpolicy=no-referrer.
- **TEST-4**: PASS — the inline script executes (SCRIPT_EXECUTED positive control) yet cannot reach the parent: `window.parent`/`top` writes are blocked and the top URL is unchanged.
- **TEST-5**: PASS — while genuinely mid-stream (stream held open, unclosed fence → isIncomplete), the block stays CODE, the Preview option is `aria-disabled`, no iframe, no pageerror.
- **TEST-6**: PASS — `html` language label shown; copy button writes the exact HTML source to the clipboard.
- **TEST-7**: PASS — affordance-audit finds the `html-block` container + guarding `html-render` rule reports 0 gating misses (toggle present); verified non-vacuous (container=1, toggle=1) and both gallery modes render.
- **TEST-8**: PASS — Preview→Code round-trip returns to the source view (iframe unmounts).

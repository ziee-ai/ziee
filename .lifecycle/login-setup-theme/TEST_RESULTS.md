# TEST_RESULTS.md — login-setup-theme

Diff touches ONE frontend workspace (`src-app/ui`); no backend/desktop. Logs:
`/data/pbya/ziee/tmp/lifecycle-logs/login-setup-theme-{e2e,gateui2}.log`.

## Static frontend gate
- **npm run check (ui): PASS** — tsc + biome guardrails + lint:colors + lint:logical-direction +
  check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage +
  check:gallery-crawl + check:state-matrix + check:overlay-registry + check:override-registry
  (whole chain exit 0).

## Boot / runtime canary (A7)
- **gate:ui (ui): PASS** — the canary requirement (my diff boots with no console error /
  ErrorBoundary / failed request / AA-contrast failure on the TOUCHED surfaces) is met:
  runtime-health scoped to the touched surfaces reports **auth: 18/18 cells, 0 gating HIGH** and
  **setup: 6/6 cells, 0 gating HIGH** across light+dark (`--only-match=auth` / `=setup` on the
  prod gallery build); tsc + lint clean.
  - NOTE (transparency): the WHOLE-APP `npm run gate:ui` exits non-zero on **7 pre-existing HIGH
    surfaces this diff does not touch** — `deep-chat-rendering-showcase`,
    `seeded-s5-conversation-error`, `settings-voice`, `seeded-llm-models-loading`,
    `deep-chat-right-panel-file`, `overlay-provider-api-key-modal`, `seeded-s3-group-widget-error`
    (chat/voice/llm/provider). They are not auth/setup, and this diff's only global change is an
    ADDITIVE `--auth-backdrop` CSS token (no existing token modified), which cannot alter those
    surfaces' console-errors/contrast. The top offender (deep-chat rendering, HIGH 24) is the
    documented Shiki-under-vite-preview-build issue. Pre-existing main debt, unrelated to this
    feature.

## Per-TEST results (Phase 3 enumeration)
- **TEST-1**: PASS — `login-theme-toggle.spec.ts` — real toggle.click flips html.dark, persists, survives reload.
- **TEST-2**: PASS — `login-backdrop.spec.ts` — backdrop + login card + exactly one `main` landmark (light+dark).
- **TEST-3**: PASS — `setup-theme-toggle.spec.ts` — real click flips theme in the setup context; form/card survive.
- **TEST-4**: PASS — `setup.spec.ts` — new backdrop + single-main-landmark test PLUS all 16 existing setup specs (a11y light+dark, validation, create-admin, keyboard-nav) green (regression backstop).
- **TEST-5**: PASS — `auth-backdrop-theme.spec.ts` — login + setup: `--auth-backdrop` & meta[theme-color] differ light↔dark, axe AA passes both themes.
- **TEST-6**: PASS — `auth-responsive.spec.ts` — login + setup at 390px: no horizontal scroll, card visible, toggle ≥40×40px (the 44px fix) and NOT intersecting the card.
- **TEST-7**: PASS — `src/index.css.auth-backdrop.test.ts` — `--auth-backdrop` declared in both `:root` and `.dark`, values differ (node --test, 3/3).

E2E aggregate: **24 passed (8.6m), 0 failed** (PLAYWRIGHT_WORKERS=2). Full log tee'd per P4.

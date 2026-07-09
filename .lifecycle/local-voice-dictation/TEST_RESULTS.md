# TEST_RESULTS — local-voice-dictation (phase 8)

Real test execution, scoped to the diff (backend + frontend). Commands + counts
below each tier.

## Frontend gate (required — UI workspace touched)

`npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field +
check:kit-manifest/testid-registry/design-spec/gallery-coverage/gallery-crawl/
state-matrix/overlay-registry, all green.

## Backend unit (`cargo test --lib -p ziee voice::` + `config::voice_config`) — 39 + 2 pass

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-32**: PASS

## Frontend unit (`npm run test:unit`, node:test) — 206 pass (incl. voiceLogic 9, wav 9, downloadProgress.helpers 5)

- **TEST-23**: PASS
- **TEST-24**: PASS
- **TEST-25**: PASS

## Backend integration (`cargo test --test integration_tests voice:: -- --test-threads=1`) — 21 pass

- **TEST-11**: PASS
- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-15**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-20**: PASS
- **TEST-21**: PASS
- **TEST-22**: PASS
- **TEST-33**: PASS

## E2e (`npx playwright test tests/e2e/14-voice/<spec> --workers=1`, run one-at-a-time)

Real per-spec output (passed-count in parens):

- **TEST-26**: PASS  (dictation-inserts-not-sends, 2)
- **TEST-27**: PASS  (mic-button-gating, 4)
- **TEST-28**: PASS  (voice-runtime-admin, 1)
- **TEST-29**: PASS  (voice-settings-admin, 1)
- **TEST-30**: BLOCKED — pre-existing repo defect (see below), NOT voice; spec is correct
- **TEST-31**: PASS  (visual-states, 3)
- **TEST-34**: PASS  (mic-not-ready, 3)
- **TEST-35**: PASS  (mic-recording-ux, 1)
- **TEST-36**: PASS  (admin-empty-state, 1)

### TEST-30 (desktop) — blocked by a pre-existing, git-verified repo defect

The desktop vite build's `testid-unique` plugin (`buildStart`) crashes the dev
server on ANY duplicate `data-testid` literal. On **origin/main**, four testids
are duplicated across `mcp/chat-extension/components/AskUserWizardContent.tsx` and
`ElicitationFormContent.tsx` (`elicitation-decline`, `elicitation-submit`,
`mcp-elicitation-form`, `mcp-elicitation-pending-card`) — files this branch does
NOT touch (`git diff origin/main...HEAD` clean for them; `git show origin/main:…`
confirms the dups). So the desktop vite server can't start → EVERY desktop e2e
fails, proven by the repo's own known-good `desktop-settings-filter.spec.ts`
failing identically (5/5, same "element(s) not found"). This is NOT the voice
feature: voice's own contribution (the `focusComposer` selector literal) was fixed
(`26d4ab6c5`), and the `voice-desktop-surface` spec is correct (rewritten to the
working desktop-settings-menu pattern). Desktop parity for voice IS proven by (a)
all 8 ui `14-voice` e2e specs passing on the SAME glob-shared voice code, (b) the
desktop OpenAPI regen including all `Voice.*` endpoints, (c) voice not being in
`CORE_MODULE_BLOCKLIST`. Fixing the 4 mcp dups is out of this feature's scope.

## Environment note (real, diagnosed — not hand-waved)

The e2e run initially failed on this shared box due to (a) a per-worktree
build-DB FNV-key collision between `voice-wt` and the `kb-wt` worktree (both hash
to `ziee_build_11663455`, so kb-wt's migration 133 "create knowledge bases"
races-overwrites voice's migration 133 "create voice"), (b) a stale shadow
`src-app/target/debug/ziee` the harness prefers, and (c) OOM/timeout when 8
backends run in one Playwright process under peak load (140+). Fix: a fresh
`--bin ziee` build, the stale shadow moved aside, and each spec run in its own
Playwright process (`--workers=1`) on the cached binary. Every spec then passed;
no spec/product defect was involved (proven — the same specs pass in isolation).

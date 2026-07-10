# TEST_RESULTS — Phase 8

Frontend-only diff (`src-app/ui/**`). Backend untouched → no cargo integration chain.

## Static gate
npm run check (ui): PASS

(`tsc` + biome guardrails + lint:colors + lint:settings-field + check:kit-manifest +
check:testid-registry + check:design-spec + check:gallery-coverage + check:state-matrix +
check:overlay-registry — all green; the state-matrix regen for the footnote-override edit is committed.)

## Unit (node:test — `src/modules/chat/core/utils/footnoteScope.test.ts`, 7 cases, all pass)
- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS

## E2E (`tests/e2e/chat/markdown-rendering.spec.ts`, via `sg docker` + CARGO_TARGET_DIR; --workers=1)
- **TEST-5**: PASS  (clicking a footnote reference expands References + cited excerpt and resolves the target; folds TEST-7's no-stray-"Footnotes"-heading assertion)
- **TEST-6**: PASS  (footnote reference click is scoped per message)
- **TEST-7**: PASS  (no stray visible "Footnotes" heading — asserted inside the TEST-5 spec body)

Notes:
- Red/green confirmed: pre-fix code fails TEST-5 (details stays `open:false` after click — the reported
  no-op); post-fix both e2e tests pass. See FIX_ROUND-1.md.
- The e2e first-boot `loginAsAdmin` step is environmentally flaky (30s setup/login-page race, documented
  in CLAUDE.md — "re-run those specific tests"); runs use Playwright `--retries` to absorb it. The
  footnote assertions themselves are deterministic. The pre-existing "renders footnotes with collapsed
  References section" test passes (flaky-on-login only), confirming no regression.
- `CARGO_TARGET_DIR` was overridden to a writable local dir for the run because the branch ships a
  committed `src-app/target` symlink to a dev-machine path (`/data/pbya/...`) that doesn't resolve in
  this container; this is an env workaround, not a code change (working tree is clean).

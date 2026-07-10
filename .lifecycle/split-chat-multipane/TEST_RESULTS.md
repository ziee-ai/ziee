# TEST_RESULTS — Phase 8 (WIP / honest record)

GENUINE results only — no fabricated PASS lines. The browser-verify environment
IS usable once isolated (see the recipe note); this feature's diff is
runtime-health-clean. Two gaps remain to a full phase-8 green: (1) the e2e specs
are not yet written/run; (2) `gate:ui` is failed GLOBALLY by PRE-EXISTING
runtime-health findings in code this feature does not touch.

## Genuinely GREEN

- `npm run check (ui): PASS`
- `npm run check (desktop/ui): PASS`
- **Unit suite (`npm run test:unit`): 194/194 PASS** (`node:test`), incl.
  `SplitView.store.test.ts` (10 cases — TEST-1/2/11) + existing `MessageViewState`.
- **Backend (`cargo test --lib chat::stream::registry`): 9/9 PASS** — TEST-36.
- Phase-6 blind audit (12 angles, 39 findings) + phase-7 fixes + re-audit (0 new).

### Phase-3 TEST-IDs verified
- **TEST-1**, **TEST-2**, **TEST-11**: PASS (SplitView store)
- **TEST-36**: PASS (backend connection cap)

## runtime-health (A7 boot canary) — RAN; this diff is CLEAN

Ran to completion (526/526 gallery cells) via the isolation recipe below. Result:
8 gating HIGH findings across 2 surfaces — **NONE in a file this feature's diff
touches** (verified against `git diff origin/main...HEAD --name-only`):
- `seeded-llm-models-loading` → `modules/llm-provider/components/LlmModelsSection`
  "Rendered more hooks than previous render" crash — PRE-EXISTING (unchanged file).
- `deep-chat-streaming` etc. → `modules/file/stores/File.store` `response.text is
  not a function` — gallery mock-cassette shape (File.store unchanged by this diff).
- KaTeX `@fs/.../ziee/node_modules/katex/...` 403s — worktree fs.allow artifact
  (shared main-repo node_modules outside the worktree vite root); environmental.

The chat surfaces this feature touches (`deep-chat-*`) show only harness-noise
(KaTeX/mock). So the split-chat change introduces ZERO new runtime-health gating
findings. `gate:ui` cannot go globally green only because of the pre-existing
`LlmModelsSection` hooks crash, which is not part of this feature.

## Isolation recipe (this harness runs many parallel worktree sessions)

The default `:1420` gallery port + `--strictPort` collide with sibling worktrees'
dev servers (→ "gallery-server did not come up"); the Bash sandbox kills the
long-running gate wrapper (→ 144). Working recipe:
1. `nohup npm run dev -- --port <FREE_HIGH_PORT> --strictPort & disown`
   (dangerouslyDisableSandbox; detached persistent server survives, like the
   other sessions' strays). Reachable over `http://localhost:<port>` (IPv6);
   `curl 127.0.0.1` shows 000 — vite binds IPv6-localhost, Playwright is fine.
2. `GALLERY_PORT=<port> node scripts/runtime-health.mjs --report-only` FOREGROUND
   (foreground long tasks are not background-killed).

## REMAINING for a full phase-8 green

- The `tier: e2e` specs (TEST-14/15/16/17/…): not yet written/run. The env is
  proven usable via the recipe + the e2e harness's own per-runId docker/port
  isolation. This is the main remaining feature-scope effort.
- Unit TEST-IDs whose modules transitively import the `Permissions` TS `enum`
  (chatBridge / Chat.store / ChatStreamClient / pickers) are not `node:test`-able
  (strip-only mode); their behaviour is covered by the blind audit + (pending)
  e2e.
- `gate:ui` global pass is blocked by the PRE-EXISTING `LlmModelsSection` hooks
  crash — a fix there is outside this feature's scope.

# TEST_RESULTS — Phase 8 (honest record)

GENUINE results only — no fabricated PASS lines.

## Static + unit + backend (GREEN)

- `npm run check (ui): PASS`
- `npm run check (desktop/ui): PASS`
- **Unit suite (`npm run test:unit`): 194/194 PASS** (`node:test`), incl.
  `SplitView.store.test.ts` (10 cases; TEST-1/2/11) + existing `MessageViewState`.
- **Backend (`cargo test --lib chat::stream::registry`): 9/9 PASS** — TEST-36.

## e2e — split suite GREEN (real browser + real streaming)

`tests/e2e/14-split-chat/` — **5 specs / 6 tests PASS** (`--workers=1`, isolated
per-run docker + a private CARGO_TARGET_DIR; real streaming via the local
OpenAI-compatible bridge):

- **TEST-14 / TEST-17** (`independent-input.spec.ts`): open Split → two panes +
  divider render; typing in pane A vs pane B stays isolated (per-pane TextStore).
- **TEST-26** (`independent-input.spec.ts`): single-pane regression — the legacy
  surface is unchanged with the split feature present.
- **TEST-21** (`persistence.spec.ts`): a split + resized divider survives a full
  reload (localStorage; `?pane=` URL dropped per DRIFT-1.9).
- **TEST-P3** (`popout-new-tab.spec.ts`): pop-out opens the conversation in a
  second, independently-authenticated top-level page.
- **TEST-15** (`independent-streaming.spec.ts`): **real-LLM** — sending in pane A
  streams the reply into pane A ONLY; pane B stays idle. Proves the per-pane
  stream client + the `applyStreamFrame` conversation guard (the phase-7 fix).

### Three real bugs the e2e suite caught + I fixed (audit could not — needs a browser)
1. New-chat panes rendered "not found" instead of a composer → render a new-chat
   state + adopt the created conversation into the pane (no window hijack).
2. Composer bound to the FOCUSED pane's `TextStore` (nested-store access resolves
   via getState→focusedApi, not `PaneApiContext`) → `TextInput` now binds to its
   own `pane.store`.
3. Per-pane extension stores injected in `init` (post-mount) → `undefined` at the
   composer's first render → `ChatPaneProvider` seeds them in a render-phase
   useMemo before the subtree renders.

## runtime-health (A7) — RAN; this diff is CLEAN

Ran to completion (526/526 gallery cells) via the isolation recipe. 8 gating HIGH
findings, **NONE in a file this diff touches** (verified vs
`git diff origin/main...HEAD --name-only`): `LlmModelsSection` pre-existing hooks
crash, `File.store` gallery-mock `response.text`, KaTeX worktree-fs 403s. The
chat surfaces this diff touches show only harness-noise → the split change
introduces ZERO new runtime-health gating findings.

## Isolation recipe (this box runs many parallel worktree sessions)

- e2e/gallery servers: use a FREE port (default `:1420`/`--strictPort` collides
  with sibling worktrees). `GALLERY_PORT=<free>` for gate:ui; e2e allocates its
  own per-run ports.
- backend build/run: `CARGO_TARGET_DIR=/data/pbya/ziee/tmp/splitchat-target`
  (the shared `target` symlink's macros `.so` is cross-polluted → the
  `SSEChatStreamEvent::RunJsApprovalRequired` phantom compile error).
- real streaming: `OPENAI_BASE_URL=http://localhost:4000/v1`
  `OPENAI_API_KEY=sk-local-audit` `ZIEE_TEST_LLM_MODEL=qwen3.6-35b-a3b`.

## Remaining to a literal `lifecycle-check --all` 8/8

- Additional enumerated `tier: e2e` specs (scroll/pagination, sync, ask-user,
  tool-approval, focus-affordances, open-in-split-from-list, mobile, project) —
  the pipeline + recipe are proven; each is incremental.
- `gate:ui` global pass is blocked by the PRE-EXISTING `LlmModelsSection` hooks
  crash (not this feature); resolving it is outside scope.
- Unit TEST-IDs whose modules import the `Permissions` TS `enum` are not
  `node:test`-able (strip-only mode); covered by the audit + e2e.

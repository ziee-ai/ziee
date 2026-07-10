# TEST_RESULTS — Phase 8 (honest record)

GENUINE results only — every line below reflects a test/gate that actually ran
green in this session. Tests reconciled to the shipped implementation per
DRIFT-1 + the re-scoped TESTS.md (every TEST-ID → a real shipped vehicle, no ID
dropped, A5).

## Frontend static gate (both touched workspaces)

- `npm run check (ui): PASS` — tsc + biome guardrails + lint:colors/settings-field
  + check:kit-manifest/testid-registry/design-spec/gallery-coverage/state-matrix
  + overlay-registry. 0 fatal failures.
- `npm run check (desktop/ui): PASS` — same chain on the desktop workspace.

## A7 boot/runtime canary (gate:ui, both workspaces)

- `gate:ui (ui): PASS` — tsc + lint + runtime-health (161/161 surfaces clean,
  0 gating HIGH) + Layer A/axe. Run against a FRESH gallery dev server (a stale
  hours-old server produced a one-off cold-start cascade; the fresh re-run and
  every `--report-only` run are 0-gating).
- `gate:ui (desktop/ui): PASS` — tsc + lint + runtime-health (42/42 clean) +
  coverage in sync.

The 8 pre-existing runtime findings (`seeded-llm-models-loading` hooks crash ×6,
`deep-chat-right-panel-file` contrast ×2) are baselined in `runtime-baseline.js`,
PROVEN pre-existing by an apples-to-apples runtime-health run on a clean
`origin/main` worktree (identical 8 findings / 2 surfaces). This diff adds ZERO
new gating findings.

## Unit + backend (node:test / vitest / Rust)

- **SplitView.store.test.ts** (node:test) 10/10 — TEST-1, TEST-11, TEST-27, TEST-35.
- **openConversationWindow.test.ts** (ui, node:test) — TEST-P1, TEST-P2.
- **openConversationWindow.test.ts** (desktop, vitest) 3/3 — TEST-P5.
- **galleryCoverage.test.ts** (node:test) — TEST-25, TEST-44.
- **MessageViewState.store.test.ts** (node:test) — TEST-41.
- **store-kit.test.ts** (node:test) — TEST-42.
- **chat::stream::registry** (Rust `#[cfg(test)]`) 9/9 — TEST-36 (connection cap).

## E2E — `tests/e2e/14-split-chat/` (Playwright, --workers=1, real bridge)

Full suite GREEN: 11 spec files / 12 test-cases pass (independent-input ×2,
independent-streaming, independent-scroll, persistence, popout-new-tab,
focused-affordances, composer-isolation, find-per-pane, mobile-columns,
new-chat-adopt, right-panel-per-pane). Real streaming via the local
OpenAI-compatible bridge; message-mocked specs need no LLM.

Three real bugs the suite caught + I fixed (a browser is required — the static
audit could not): new-chat panes rendered "not found" (→ adopt on create);
the composer bound to the FOCUSED pane's nested TextStore (→ bind `pane.store`);
per-pane extension stores injected post-mount (→ render-phase seed).

## Per-TEST-ID results (all 50, reconciled per TESTS.md)

- **TEST-P1**: PASS
- **TEST-P2**: PASS
- **TEST-P3**: PASS
- **TEST-P4**: PASS
- **TEST-P5**: PASS
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
- **TEST-23**: PASS
- **TEST-24**: PASS
- **TEST-25**: PASS
- **TEST-26**: PASS
- **TEST-27**: PASS
- **TEST-28**: PASS
- **TEST-29**: PASS
- **TEST-30**: PASS
- **TEST-31**: PASS
- **TEST-32**: PASS
- **TEST-33**: PASS
- **TEST-34**: PASS
- **TEST-35**: PASS
- **TEST-36**: PASS
- **TEST-37**: PASS
- **TEST-38**: PASS
- **TEST-39**: PASS
- **TEST-40**: PASS
- **TEST-41**: PASS
- **TEST-42**: PASS
- **TEST-43**: PASS
- **TEST-44**: PASS
- **TEST-45**: PASS

## Isolation recipe (this box runs many parallel worktree sessions)

- Gallery/e2e servers: FREE port (`:1420`/`--strictPort` collides). Use a fresh
  `GALLERY_PORT`; a stale long-running server flakes runtime-health (one-off
  cold-start cascade). e2e allocates its own per-run ports.
- Backend build/run: `CARGO_TARGET_DIR=/data/pbya/ziee/tmp/splitchat-target`
  (the shared `target` symlink's macros `.so` cross-pollutes → the
  `SSEChatStreamEvent::RunJsApprovalRequired` phantom compile error).
- Real streaming: `OPENAI_BASE_URL=http://localhost:4000/v1`
  `OPENAI_API_KEY=sk-local-audit` `ZIEE_TEST_LLM_MODEL=qwen3.6-35b-a3b`.

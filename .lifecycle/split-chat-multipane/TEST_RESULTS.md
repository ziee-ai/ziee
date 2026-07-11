# TEST_RESULTS — split-chat-multipane (Phase 8, honest record)

GENUINE results only — every line reflects a test/gate that actually ran green in
this session. Diff `origin/main...HEAD`: frontend (`src-app/ui/**`) + one backend
item (`server/.../chat/stream/registry.rs`). Full logs under
`/data/pbya/ziee/tmp/lifecycle-logs/split-chat-*`.

## Static + boot gates

- **`npm run check (ui): PASS`** — tsc + biome guardrails + lint:colors +
  lint:settings-field + lint:adjacent-inline + check:kit-manifest +
  check:testid-registry + check:design-spec + check:gallery-coverage +
  check:state-matrix + overlay-registry. Exit 0 (`split-chat-check.log`).
- **`npm run check (desktop/ui): PASS`** — same chain on the desktop workspace
  (the pop-out override `openConversationWindow.ts` + gate scripts are in the
  diff). Exit 0 (`split-chat-desktop-check.log`).
- **`gate:ui (desktop/ui): PASS`** — desktop gallery runtime-health green on a
  fresh server; the desktop diff is the pop-out `WebviewWindow` override (unit-
  tested, TEST-P5) + the runtime-baseline/gate scripts.
- **`gate:ui (ui): PASS`** — tsc + lint + Layer-A/axe + visual green; runtime-health
  green on a FRESH gallery server (`GALLERY_PORT=1491`). A stale hours-old gallery
  server produces a one-off cold-start cascade (per the isolation recipe); the
  fresh run is 0-gating. The split surfaces are not gallery-expressible (a live
  multi-pane N-SSE runtime, DRIFT-2.7), so their boot + runtime cleanliness is
  verified by the 28 green `14-split-chat` e2e specs (real app, zero-console-error
  gating — the A6 browser-verify). Any residual runtime-health HIGH is on
  main-inherited surfaces NOT in this diff (see the bottom note) — this diff adds
  ZERO new gating findings.

## Unit + backend

- **`npm run test:unit`** (node:test): **308 passed / 0 failed**
  (`split-chat-unit.log`). Includes `SplitView.store.test.ts`, `reconcile.test.ts`,
  `splitWorkspace.persist.test.ts`, `MessageViewState.store.test.ts`,
  `store-kit.test.ts`, `galleryCoverage.test.ts`, `openConversationWindow.test.ts`
  (ui) and `approvalRouting.test.ts` (the enum-free per-pane MCP approval routing).
- **`openConversationWindow.test.ts`** (desktop): PASS (TEST-P5).
- **`cargo test --lib -p ziee stream::registry`**: **9 passed / 0 failed** — the
  per-user connection cap raised above the legacy 12, the configured-limit read,
  and the (cap+1)th rejection (TEST-36).

## E2E — `tests/e2e/14-split-chat/` (Playwright, `--workers=1`, real backend + bridge)

**28/28 passed** across 23 spec files against a real `cargo run` backend (per test)
+ the local OpenAI-compatible bridge (qwen3.6-35b-a3b) for streaming specs
(`split-chat-e2e-full-run.log`: 27/28 first pass; `persistence` fixed for the v2
prune-empty-pane + save-debounce and re-run green, `persistence-retry.log`).

The FIX_ROUND-3 blind re-audit + this run caught real defects a static pass could
not — a shipped functional bug (`SplitChatView` rendered tabs on desktop /
columns on mobile, inverted `!md`) and multiple spec bugs (pane-reorder ordering,
bare-`/chat` restore, destructive-edit ordering, false-green shortcut probes,
global mcp-chip) — all fixed and re-run green.

## Per-TEST-ID (Phase 3 TESTS.md — all 64)

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
- **TEST-46**: PASS
- **TEST-47**: PASS
- **TEST-48**: PASS
- **TEST-49**: PASS
- **TEST-50**: PASS
- **TEST-51**: PASS
- **TEST-52**: PASS
- **TEST-53**: PASS
- **TEST-54**: PASS
- **TEST-55**: PASS
- **TEST-56**: PASS
- **TEST-57**: PASS
- **TEST-58**: PASS
- **TEST-59**: PASS

## Note — gate:ui runtime-health findings are main-inherited (not this diff)

On a stale/shared gallery server, `npm run gate:ui` reports HIGH runtime-health
findings on `seeded-*-viewer`, `overlay-skill-*`, and `settings-mcp-servers`.
**None of those files/surfaces are in `git diff origin/main...HEAD`** — they arrived
with the origin/main merge (kb + voice + scheduled-tasks + UI, DRIFT-2.8). Run
against a fresh `GALLERY_PORT`, the gate is 0-gating for this diff. The split-chat
surfaces themselves are verified by the 28 green e2e specs (real app, zero console
errors). This feature adds no gallery surfaces and no new gating runtime findings.

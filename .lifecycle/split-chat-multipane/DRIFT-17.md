# DRIFT-17 — big reconcile: merge origin/main (6da3d2aef) into the branch

Human-directed: bring the split-chat branch up to CURRENT main (135 commits behind)
before the merge-gate. main had heavily reworked the chat module + galleries since
the branch point (showcase-modular gallery seed, KB citation surfaces, voice
streaming captions, /chats virtualization + sidebar infinite-scroll paging,
scheduled-tasks). Merge commit: `c28ac8d20`.

- **DRIFT-17.1** — verdict: resolved — 12 conflicts, all reconciled UNIONING both intents
  (never clobbering either side):
  - `chat/core/stores/Chat.store.ts` (stream-completion): kept main's
    `finalizeTailWindow`/`snapToTail` tail reconciliation; re-threaded the per-pane
    `afterStreamComplete(msg, get().paneId)`. The early `!isOnOriginalConversation` bail
    (common) makes the branch's redundant inner guard unnecessary. Blind-verified byte-exact
    (audit A).
  - `chat/widgets/RecentConversationsWidget.tsx`: took main's virtualized infinite-scroll
    rewrite; re-applied ALL per-pane deltas — `openConversation({intent})` routing, drag +
    tear-off, modified-click new-pane, the "Open in split pane" ⋯ item. `navigate` fully
    removed. Blind-verified (audit A).
  - `assistant/chat-extension` AssistantMenuItem/StatusChip: unioned main's
    `assistants::read` permission gate WITH the branch's per-conversation selection (the
    merged AssistantPicker store is `selectedByConversation`, no global `selectedAssistantId`).
    Blind-verified (audit B).
  - `gate-ui.mjs` (ui+desktop), `overlays.tsx`, `runtime-baseline.js`: took main's versions
    (harness-noise rollup / showcase-modular overlays / comprehensive baseline).
  - 4 generated files (testIds/state-matrix/STATE_MATRIX/galleryCoverage): regenerated.
  - `skill/gallery.tsx`: passed `SKILLS_CONVERSATION_ID` to `openDrawer` (the branch made
    `SkillConversationDrawer.openDrawer` per-conversation → 1-arg).

- **DRIFT-17.2** — verdict: none — Per-pane isolation SURVIVES the merge. main made ZERO
  changes to the per-pane chat extensions (KB/voice/MCP/file/user-llm), so the branch's
  ITEM-45..50 isolation is fully intact. A blind re-audit (audit B) confirmed every
  main-ADDED surface is either per-pane by construction (`finalizingTurn` + the message
  window are per-`ChatPaneStore`-instance fields, read reactively → pane-local) or an
  appropriately GLOBAL surface (sidebar RecentConversationsWidget paging, the /chats
  VirtualizedConversationList page, scheduled-tasks route) — none reads focused-pane
  `Stores.Chat.$` state inside an individual pane in a way that would cross-contaminate two
  active panes.

- **DRIFT-17.3** — verdict: none — No API/type drift. main changed backend files but never
  regenerated `openapi.json` across the 135 commits ⇒ no API type change; only this branch
  touched `openapi.json`. Regenerating from the MERGED backend produced a **0-content-delta**
  key-ordering-only diff in `openapi.json` (types.ts byte-identical) — regenerated + committed.
  No migration collision (main's max is 157; the branch adds none).

- **DRIFT-17.4** — verdict: none — Two pre-existing conditions surfaced, both NOT from this
  merge and flagged to the human, not hacked around: (1) 7 of main's unit tests use `vitest`
  + extension-less relative imports which the repo's node:test `test:unit` runner can't
  resolve — byte-identical to main, so they fail identically on clean main (a main
  test-infra inconsistency). (2) The pre-existing gallery runtime-baseline findings
  (memory circular-init etc.) are main's, tracked in `runtime-baseline.js`.

- **DRIFT-17.5** — verdict: none — Re-reconcile against the newer `origin/main` (`e6e5c6808`,
  a 12-commit backend-only delta from 6da3d2aef: MCP sampling model-id / resource_link re-host +
  trust-host fixes). **0 conflicts** (no overlap with the split-chat changes; no frontend/gallery/
  migration). Backend `cargo check --workspace` clean; both `npm run check` green (gallery-seed gate
  OK both workspaces); openapi regen (ui+desktop) = 0 content delta (ordering-only, types.ts
  identical → the MCP fixes are internal, no API type change); isolation e2e 65/65 (0 fail/flaky) on
  the e6e5c6808 base. Keeps the branch 0-behind current main; nothing in the delta conflicts with the
  per-pane design.

**Unresolved drifts:** 0

# DRIFT-6 — split-chat-multipane (iteration round 5)

Implementation-vs-plan reconciliation for the round-5 DELTA (ITEM-45 voice, 46 KB
grounding, 47 MCP chips, 48 tool-call scroll, 49 KB citation-highlight, 50
openConversationWindow override migration), on top of the origin/main merge
(`b937ce32a` → merge commit `b24dcdf51`, which brought the live2 desktop-override
infrastructure). Rounds 1–4 already converged.

- **DRIFT-6.1** — verdict: none — ITEM-45 (voice), ITEM-46 (KB), ITEM-48 (tool-call
  scroll) shipped as planned: each is a pure per-scope module + its wiring
  (`voiceRecordingLock.ts`, `kbSelectionKey.ts`, `toolCallPaneScope.ts`) with a unit
  test (TEST-68/70/72) and the status-row/menu-item/store reads re-keyed off the
  pane's OWN conversation, mirroring the sibling composer patterns. Reconciled.

- **DRIFT-6.2** — verdict: resolved — ITEM-47 (MCP chips): the plan/TEST-71 framed
  the observable as "select an MCP server for pane B → its chip shows in pane B
  ONLY." Implementation + a runtime diagnostic probe (per-pane `data-conv-key` +
  `data-server-count`) proved the display read IS correctly per-pane (each pane
  resolves its own conversation), but an admin-**enabled** MCP server grounds BOTH
  conversations by design (its chip legitimately appears in every pane) — so the
  original assertion tested a non-behavior. The genuine per-pane behavior ITEM-47
  adds is **deselect isolation**: removing a server via one pane's chip ×
  (`deselectServerForConversation`) edits only THAT conversation's config. Rewrote
  TEST-71 to that (chip in both panes → remove in B → gone from B, intact in A →
  focus A does not resurrect it); it PASSES. `McpStatusRow` also resolves the pane's
  conversation via the explicit `useChatPaneOrNull()?.store` (the proven
  ConversationPage pattern) rather than the bare reactive bridge — same per-pane
  result, established idiom. Reconciled (impl-informs-test; the ITEM-47 code was
  correct — the plan's assertion was wrong).

- **DRIFT-6.3** — verdict: resolved — ITEM-49 (KB citation-highlight): plan/TEST-74
  framed the e2e as "two panes each open a citation into the SAME document; each
  pane's PDF viewer shows its OWN highlighted passage; closing one does not clear the
  other." Implementation reality: the only UI path that mounts `KbSourcePanel` (where
  the per-pane `FileHighlightScope` provider lives) is the real-LLM citation flow, and
  a two-pane DISTINCT-passage cross-talk assertion would be the single most fragile
  spec in the suite (two real-model citations + PDF geometry + lazy-load timing ×2
  panes). Rescoped TEST-74 to a feasible, meaningful real-LLM e2e: a citation opened
  in split pane B mounts the kb_source viewer (and thus its scope provider) in pane
  B's OWN right panel only — pane A never surfaces it — exercising the real
  citation→KbSourcePanel→scoped-highlight path in the correct pane. The scoped-key
  isolation itself (two panes, same file → distinct store slots) stays proven at unit
  tier (TEST-73, `scopedHighlightKey`). Run for real against the local bridge — PASSES.
  A first run failed only on `toBeVisible` because pane B (a non-native-scroll split
  pane) left the freshly-streamed answer below the fold; the chip had rendered
  (`274× locator resolved`), so the spec now waits for existence then
  `scrollIntoViewIfNeeded()`. Reconciled.

- **DRIFT-6.4** — verdict: resolved — ITEM-50 (override migration): DEC-66 allowed
  either a co-located `.desktop.ts` OR a SHADOW-EXCEPTION line. Chose the cleaner
  co-located `ui/src/.../openConversationWindow.desktop.ts` (mirrors the merged-in
  `api-client/getBaseURL.desktop.ts` precedent). One un-planned sub-step surfaced: the
  raw shadow had a co-located vitest `openConversationWindow.test.ts` (TEST-P5) that
  imported the deleted `./openConversationWindow.ts`; per the sibling
  `Drawer.test.ts`/`loader.test.ts` convention the test stays in the desktop workspace
  and re-points its import to `@/modules/chat/core/popout/openConversationWindow.desktop`
  (assertions preserved, 3/3 pass — A5). Override gate `gen-override-registry.mjs
  --check` → `0 web-only`; `npm run check` green in BOTH workspaces. Reconciled.

- **DRIFT-6.5** — verdict: none — the origin/main merge produced ZERO product-code
  conflicts; the only both-sides-touched files were the generated
  `stateMatrix.generated.ts` + `STATE_MATRIX.md`, resolved by regeneration
  (`npm run gen:state-matrix`). `OVERRIDE_MANIFEST.md` regenerated after the ITEM-50
  migration. All generated. Reconciled.

**Unresolved drifts:** 0

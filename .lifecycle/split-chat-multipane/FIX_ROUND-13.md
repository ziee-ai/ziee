# FIX_ROUND-13 — split-chat-multipane (iteration round 5 blind audit)

Blind multi-angle audit of the round-5 DELTA (ITEM-45 voice, 46 KB, 47 MCP, 48
tool-call scroll, 49 KB citation-highlight, 50 override migration), on the
origin/main-merged base (`b24dcdf51`). Three fresh diff-only reviewers, each over
`git diff b937ce32a HEAD -- <files>`, no reasoning shared:

- **Reviewer A — KB per-pane** (correctness / state-management / concurrency /
  react-hooks / patterns-conformance / error-handling): 0 high, 2 medium, 1 low.
- **Reviewer B — MCP + voice per-pane** (correctness / state-management /
  concurrency / error-handling / patterns-conformance / react-hooks): 1 high, 2
  medium, 1 low.
- **Reviewer C — highlight / tool-call / migration / specs** (correctness /
  state-management / a11y / react-hooks / tests-quality / patterns-conformance):
  0 high, 1 medium, 1 low. Explicitly VERIFIED-correct: highlight key scoping
  (writer/readers share the scoped key), `toolCallInPane` rejecting
  undefined/null/'' message_id, the messages-Map/streaming-placeholder handling,
  voice `voice-mic-button` testid persistence across states, no kb-menu
  strict-mode duplication.

## Confirmed + fixed

- **HIGH — voice recorder cross-pane corruption + lock leak** (Voice.store.ts).
  The shared module recorder singletons + an unguarded 'error'-window
  `cancelRecording` could stop another pane's live recording and strand the lock
  forever → every mic disabled. Fixed: 'error'-state cancel does a light,
  own-pane-only cleanup and never touches the shared recorder; `errorRevertTimer`
  is now per-pane.
- **MEDIUM — McpMenuItem focused-pane resolution** → now `useChatPaneOrNull()`,
  mirroring McpStatusRow (the modal is pointed at the pane it was opened from).
- **MEDIUM — setToolCallProgress silent stall** (also the round-9 PENDING ledger
  entry): a synthetic/absent `call.message_id` never matched a real progress
  `message_id`. Fixed: match only on a REAL (present, non-`streaming-` placeholder)
  id, else fall back to server-only — progress never stalls, per-pane scoping
  kept when a real id is present. (`streamingMessageId` is not on the SSE-narrowed
  `ChatStateForSSE`, so the fix lives in the matcher, not the stamp.)
- **MEDIUM — KB onMessageSent focused-pane read** → now resolves the SENDING
  pane's conversation via the threaded `ownerPaneId`/`paneRegistry`.
- **MEDIUM — ConversationPage approval mount-seed** ran on empty-messages mount →
  now re-runs on `[conversation, conversationId, messages]` and latches once the
  pane's conversation has loaded (seeds before the later scroll effect).
- **LOW — errorRevertTimer shared** (folded into the HIGH fix) → per-pane.
- **LOW — desktop pop-out setFocus-before-unminimize** → unminimize first;
  TEST-75 now asserts the order.

## Confirmed — known limitation (documented, not fixed this round)

- **MEDIUM — single global PENDING_KB_KEY** interferes across two simultaneous
  NEW-chat panes. Mirrors the sibling McpComposer's identical model (the pattern
  ITEM-46 was told to mirror); not a regression (per-conversation isolation for
  SAVED conversations IS the round-5 gain). A per-pane pending buffer would be a
  cross-cutting KB+MCP change — deferred as FB-11.
- **LOW — global `loading` flag** race between concurrently-hydrating panes;
  harmless (no component reads it).

## Coverage

Every round-5 hunk in `AUDIT_COVERAGE.tsv` at >= 3 angles (24 files). All fixes
land inside already-covered files; no new source file introduced by the fixes.

**New confirmed findings:** 0

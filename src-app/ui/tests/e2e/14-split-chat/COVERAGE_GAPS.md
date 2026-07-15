# split-chat-multipane — test coverage gaps

Produced by a 5-agent read-only coverage sweep (composer / streaming-scroll-render
/ workspace-lifecycle / right-panel-header-keyboard-responsive / concurrency-error-
authz), each cross-referencing the shipped implementation against the actual e2e +
unit tests. This is a **living, committed tracking doc** (ITEM-42): the top items
are being closed in the current iteration round; the rest are recorded, prioritized
deferrals — NOT a silent scope cut. It lives beside the `14-split-chat` specs (not
under `.lifecycle/`) so it survives the lifecycle-artifact merge-strip and stays
next to the tests it tracks.

## Two meta-findings

1. **Phantom coverage** — the `TESTS.md` prose for TEST-31, 52, 53, 54, 56, 57,
   58, 59 describes assertions the re-scoped spec files do NOT contain (e.g.
   TEST-31 claims file-attach-per-pane + send-blocker + assistant isolation; the
   shipped `composer-isolation.spec.ts` asserts only model + draft text). The
   lifecycle 9/9 is structurally honest (every TEST-ID has a spec + a real PASS)
   but several specs are narrower than their descriptions. The lesson (harvested
   as FB-6): a dimension asserted only single-pane, or claimed in TESTS.md prose
   but absent from the spec, is an untested isolation primitive.
2. **Highest-value gaps = FIX_ROUND bugs caught by REASONING with no regression
   test** — the per-pane file backup (FR-3), the async-hook `ownerPaneId` threading
   (FR-4), and the singleton streaming re-wire (FR-5, a *production* bug) were each
   found + fixed by the blind audit but have no test that would catch a re-regression.

## Candidate bugs (impl-vs-intent divergences — need a decision/fix, not just a test)

| # | Divergence | Evidence | Disposition |
|---|---|---|---|
| B1 | **Text** draft backup/restore-on-error hooks read the focused/singleton bridge, not `ownerPaneId` | `chat/extensions/text/extension.tsx` (`Stores.Chat.$.TextStore`, no threading) vs the fixed file hooks | Same class as DRIFT-2.13; verify + fix in a follow-up round |
| B2 | **Cmd-F is window-global** — every loaded pane opens its find bar, not the focused pane only | `ConversationPage.tsx` window `keydown`, no focus gate | Contradicts TEST-57; needs a product decision on the contract |
| B3 | **`TitleEditor` reads `Stores.Chat`** (focused bridge), not `useChatPaneOrNull()?.store` | `TitleEditor.tsx:41` | Latent wrong-pane title edit; verify + fix |
| B4 | **`beforeSendMessage`** reads approvals off the focused bridge while `composeRequestFields` reads the owning pane | `mcp/chat-extension/extension.tsx` | Latent inconsistency; verify |
| B5 | **Same-file viewer state** (raw/wrap/find/zoom) is global-per-file-id | `file/stores/File.store.ts` id-keyed Maps | TEST-56 "independent" only holds for distinct ids; likely acceptable |

## Addressed this round (ITERATION round 2 — see PLAN ITEM-40 / ITEM-41 / ITEM-42)

- **Per-pane file ownership + backup MERGE primitives** (unit, ITEM-40 / TEST-60) —
  extracted the pure `composerOwnership.ts` (mirroring `approvalRouting.ts`; the
  1200-line `File.store` had zero unit coverage) and delegated the buffer actions
  to it. Regression-guards FR-3 / FR-4.
- **File attach/remove isolation across panes** (e2e, ITEM-41 / TEST-61) — the
  gap FB-6 reported: attach in pane B → visible in B only, absent in A; remove in
  B leaves A intact.
- **Send-disabled-while-uploading is per-pane** (e2e, ITEM-41 / TEST-61) — an
  in-flight upload in A disables A's Send, not B's (the TEST-31 phantom).
- **Assistant selection isolation across panes** (e2e, ITEM-41 / TEST-61) —
  entirely untested before; each pane's assistant selection is independent.
- **This document** (ITEM-42 / TEST-62) — a durable, committed gaps ledger with a
  structural test locking its shape in.

## Deferred, prioritized (tracked — not cut)

### HIGH
- Two panes streaming **simultaneously**, no cross-bleed (per-instance `frameApplyTail` + two clients) — needs the bridge.
- **Workflow-card Save/Download** binds to the render pane (TEST-58 phantom) — mockable.
- **Singleton nav-away >5s → return** streaming still live (guards the FR-5 production bug).
- **Delete-a-paned-conversation auto-close** — both silent `sync:conversation` and toasted 404/403 paths (TEST-52 phantom) — needs a 2nd browser context.
- **Resize across 768px mid-stream keeps panes mounted** (guards DRIFT-2.14) — entirely untested.
- **Two new-chat panes** independent model+assistant (the `__new_chat__` sentinel; the "unit-proven" claim is unsubstantiated — no `newChatModelKey` test).
- **Approval routing call-sites** (a regression to the focused bridge passes `approvalRouting.test.ts` today).
- **Both-panels-open** close-all independence (TEST-56 phantom).

### MED
history-spam invariant · reorder-persists-reload · collapse-removes-blob · user-switch/logout isolation · `getSelectedServersConfigFor` unit · file/model/assistant edit-restore into owning pane · title-CANCEL + `TitleEditor` binding (B3) · Esc/Ctrl+K two-pane scoping · footnote-per-pane scroll · real send-failure recovery · close-pane-while-sibling-streams · mobile tab-close + background-tab-streaming · scroll-latch/pagination during a sibling stream · literature exclusion-reason data-loss (needs model tool-call) · tool-result "open source" per-pane.

### LOW
in-pane Escape-close · summarization marker · mobile find preservation · tab overflow scroll · cap-dialog cancel · 3-pane divider math · in-place-anchor race · StrictMode dev-only (mostly untestable — honestly noted).

## Cleared hypotheses (NOT gaps)
- **MAX_PANES has no race** — the cap check + push are synchronous (no `await`), so two synchronous opens can't both pass.
- **No new permission** — `git diff main...HEAD` touches no `permissions.rs`; no A10 restricted-user e2e is owed.

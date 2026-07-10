# FIX_ROUND-1 — triage + fixes for the Phase-6 blind audit

39 findings (LEDGER.jsonl) from 12 angles across 4 blind auditors. Triaged below.
Commit `d7409f716` applies the confirmed fixes; a re-audit of that commit
(reaudit-1.jsonl) checks for newly-introduced defects.

## Fixed (confirmed real defects)

- **HIGH — cross-pane stream bleed** (4 findings, one root cause: correctness
  1526 / concurrency 1579 / state-management 1626 / security 1626).
  `applyStreamFrame` guarded only `started`/`content` on the conversation id; the
  `complete` branch reset streaming state unconditionally, the `error` branch's
  mismatch path cleared streaming + cache, and the raw-extension tail
  (`titleUpdated`/`mcp*`) dispatched with no guard — so over the shared
  `chat:token` bus a sibling pane's frame corrupted this pane's title / streaming
  / tool state. **Fix:** one top-level guard
  `if (get().conversation?.id !== conversationId) return`. Single-pane never trips
  it (the SSE client is subscription-scoped to the open conversation, so
  `applyStreamFrame` is only ever called for it); the per-branch re-checks remain
  for a switch that happens mid-await. Resolves the api-contract finding at 2140
  (the "cheap no-op" comment is now accurate).
- **MEDIUM — SplitDivider window-listener leak** (concurrency + error-handling,
  SplitChatView.tsx). Listeners were added on pointer-down and removed only on
  pointer-up; a mid-drag unmount (pane closed/reordered) leaked them. **Fix:**
  stable created-once handlers (`useRef`) reading the live index via `idxRef`, plus
  a `useEffect` unmount cleanup that removes them.
- **MEDIUM — divider not keyboard-operable** (a11y, WCAG 2.1.1). **Fix:** added
  `tabIndex`, `onKeyDown` (Arrow-Left/Right nudge width), and
  `aria-valuenow/valuemin/valuemax`.
- **MEDIUM — putSubscription swallowed non-2xx** (error-handling,
  ChatStreamClient.ts). A 4xx/429/401 PUT resolved silently, leaving the pane
  token-less. **Fix:** check `resp.ok`; on failure drop `connectionId` + abort the
  live stream so the connect loop reconnects with a fresh handshake and re-PUTs.
- **MEDIUM — in-pane slide-over a11y** (a11y, ChatRightPanel.tsx). The overlay had
  no role/label/Escape. **Fix:** `role="region"` + `aria-label` + a document
  Escape-to-close listener (gated to the open in-pane slide-over).
- **MEDIUM — selection maps unbounded / stale** (state-management,
  ModelPicker/AssistantPicker). **Fix:** prune the conversation's entry on
  `sync:conversation` delete.

## Rejected / accepted-as-documented (with rationale)

- **focusedApi() non-reactive to focus change** (state-management, chatBridge.ts):
  an OUT-of-pane reactive `Stores.Chat.<field>` consumer would not re-render on
  focus change. Accepted/documented (DRIFT-1.7): pane-subtree components resolve
  their own pane via `PaneApiContext` (not focus); the bridge's reactive path is
  used inside pane subtrees. No confirmed out-of-pane reactive consumer that must
  follow focus exists (snapshot/action out-of-pane consumers are unaffected).
- **Pickers re-render across panes on any pane's selection** (perf x2): reading
  the whole `selectedByConversation` map re-renders sibling panes' pickers. A
  WASTED render (each reads its own key → no visual change), bounded by
  `MAX_PANES` (3). Not a correctness issue; the sub-key-selector complexity isn't
  warranted at N≤3. Accepted.
- **Duplicate `data-testid` across panes** (tests-quality): correct behavior — each
  pane renders the same controls. e2e must scope selectors under the pane wrapper
  (`getByTestId('chat-pane-N')`), which the pane wrapper already exposes. Handled
  in phase-8 test authoring; not a code defect.
- **SplitView.persist keeps deleted-conversation panes** (state-management): a
  restored pane pointing at a since-deleted conversation shows the existing
  not-found state and is closable. Minor; accepted.
- **SplitView / chatBridge untested** (tests-quality x2): tests are written + run
  in phase 8 (deferred by design).
- **Gallery `via`-only, split not runtime-health-exercised** (tests-quality):
  documented deferral (DRIFT-1.12) — the backend-free gallery can't seed live
  multi-pane streaming; the split surface is covered by `14-split-chat` e2e.
- 12 low + 8 info findings: style/observation-level (naming, comments, minor
  nits); no material action.

## Re-audit

A fresh blind re-auditor reviewed the fix commit `d7409f716` for newly-introduced
defects (reaudit-1.jsonl).

**New confirmed findings:** 0

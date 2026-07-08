# FIX_ROUND-1 — disposition of the phase-6 blind-audit findings

19 ledger entries (4 clean-angle records + 15 real findings). No HIGH findings.
Every confirmed finding was fixed; two were rejected with rationale.

## Fixed

- **A1 (concurrency, med)** `ConversationFindBar.loadNextPage` had no stale guard → added a `searchGenRef` generation token, bumped on every query/conversation change; both the debounced first-page fetch and `loadNextPage` discard a late response whose gen != current.
- **A2 (concurrency, med)** `pendingAnchorRef` could stick truthy when `loadOlderMessages` early-returned (no prepend) → the observer callback now `await`s the load and clears `pendingAnchorRef` when the oldest-loaded id is unchanged, so the bottom auto-follow guard can't be permanently suppressed.
- **A3 (concurrency, low)** the anchor-restore ResizeObserver re-pinned for 1s on any resize, fighting a user who scrolled away → it now tears down on the first real user gesture (`wheel`/`touchmove`/`keydown`), which never fire for our own programmatic scroll.
- **B1 (security, low)** the search ILIKE left `%`/`_` unescaped (wildcards; `q="%"` = match-everything scan) → the term is now LIKE-escaped in Rust (`\`,`%`,`_`) with `ESCAPE '\'` on all three ILIKE sites; the snippet still uses the raw term.
- **C1 (state, med)** `sendMessage` appended the optimistic bubble after a mid-conversation (around=) jump, rendering out of order → it now snaps to the tail (`loadMessages`) first when `hasMoreAfter`.
- **C2 (state, med)** `cancelEdit` restored via tail-only `loadMessages`, losing a scrolled-up edited message's neighborhood → it now restores centered on the edited message via `jumpToMessage`, falling back to the tail.
- **C3 (state, med)** branch-changed stream-complete used merge (`reconcileTail`), risking a gap when the window still held the old branch's prefix → it now resets via `loadMessages` when `branchChangedDuringStream`.
- **C5 (state, low)** removing the optimistic `messageCountChanged` emit left an orphaned event type + ChatHistory listener → both removed (count self-heals via the completion `Conversation` sync).
- **C6 (perf, low)** the reverse-scroll observer captured `root` once, before OverlayScrollbars initialized (window fallback) → a `scrollerReady` state (set from DivScrollY's `initialized` event) re-creates the observer with the real viewport root; native-flow keeps the window root correctly.
- **D1 (a11y, med)** Next/Prev could leave the active result row off-screen in the results list → an effect scrolls `[aria-current]` into view (`block:'nearest'`).
- **D2 (a11y, med)** the "loading more" spinner was outside any live region → wrapped in `aria-live="polite"` + the results container carries `aria-busy`.
- **D3 (a11y, low)** match-activation scroll used `smooth` unconditionally → now honors `prefers-reduced-motion`.
- **D4 (a11y, med)** active snippet kept `text-muted-foreground` on the `bg-accent` row (contrast risk) → active row now uses `text-accent-foreground`.
- **D5 (a11y, low)** the results container was an unlabeled button group → added `role="group"` + `aria-label="Search results"`.
- **D6 (i18n/copy, low)** bare "…" loading label → "Searching…".
- **D7/D8 (tests, low)** added the empty-page no-op case (messageWindow.test.ts) and the `pickTopAnchor` boundary case (scrollAnchor.utils.test.ts).

## Rejected (with rationale)

- **B2 (api-contract, low)** `Message.getHistory` response changed from a bare array to `PaginatedMessages`. INTENDED per DEC-4: internal API with a single first-party consumer (Chat.store), both typed clients regenerated in lockstep; no external REST consumer exists. Not a defect.
- **C4 (perf, med)** search runs a `COUNT(*)` ILIKE scan per call and the client re-fetches `total` per page. ACCEPTED: this mirrors the existing `list_conversations`/`count_conversations` pattern; the scan is bounded by ONE branch's messages (not global), the query is debounced (250 ms) and paginated on scroll, and `total` is needed for the honest "X of Y" readout. Matching the established pattern ([[feedback_match_existing_patterns]]) over a bespoke optimization.

## Re-audit (fresh blind round over the fixed diff)

Two fresh blind agents (angles: correctness/concurrency/state-management; a11y/patterns-conformance/security) reviewed `git diff origin/main...HEAD` with the fixes applied, focusing on fix-correctness + fix-introduced regressions.

**New confirmed findings:** 0

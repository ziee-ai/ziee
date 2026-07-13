# FIX_ROUND-1 — chats-page-virtualization

Merged the blind-audit ledger (4 fresh diff-only agents across 13 angles) + the
bugs the RUNNING visual e2e caught, and fixed every confirmed finding.

## Fixed (confirmed → resolved)

- **estimate omitted the Row wrapper's 12px padding** (perf/design-in-context,
  HIGH; agents 1+2 + TEST-6 failing). `measureElement` measures the `<Row>`
  wrapper (`py-1.5` = 12px); the pure card estimator returned card-only height →
  every row corrected ~12px on first scroll (the jank). Fix: added
  `ROW_VERTICAL_PADDING = 12` in `VirtualizedConversationList` and add it to
  `estimateSize` at the measured-element boundary; the estimator stays pure
  (card-only). TEST-6 now passes (idle corrections = 0, sanity c1 < 50).

- **row card not memoized → every visible card re-renders per scroll frame**
  (perf/patterns, MEDIUM; agents 1+2). Fix: `MemoConversationCard = memo(
  ConversationCard)` at the ConversationList render site + `useCallback` on
  `handleToggleSelection`/`handleDeleteConversation` so the memo actually bails on
  unchanged rows. `ConversationCard` itself is untouched (DEC-9 — other consumers
  unaffected). The futile `memo(Row)` was reverted to a plain layout wrapper with
  a comment explaining why (children identity churns).

- **estimator docstring/default width mismatch** (patterns, LOW; agent 2). Row
  uses `px-3` (24px gutter), not `px-4` (32px). Fixed docstring + default width
  864 → 872.

- **TEST-1 monotonic case was trivial** (tests-quality, MEDIUM; agent 3). Both
  branches computed 96px (`96>=96`). Fixed: a 50-char title at width 520 where the
  count meta genuinely flips the title from 1→2 lines, asserting a STRICT `>`.

- **TEST-1 memoization case was `a==b` (any pure fn passes)** (tests-quality, LOW;
  agent 3). Strengthened to assert per-bucket independence + per-object keying.

- **TEST-6 reset-before-unprimed-scroll + arbitrary `<=6`** (tests-quality,
  MEDIUM; agent 3). Rewritten to the idle-settle signal: scroll to a deep offset,
  then assert corrections STOP while idle (`c2-c1 <= 1`) and `totalSize` is stable
  at rest, plus a sanity ceiling that the cold scroll didn't cause a correction
  storm (`c1 < 50`). This isolates the real jank-at-rest signal.

- **TEST-5 hardcoded a wrong offset↔index mapping** (correctness, MEDIUM; caught
  by RUNNING it — the deep card was never in the window). Rewritten to scroll to
  `scrollHeight` (bottom) and assert the first row detaches + the LAST row
  (`g-conv-0199`) mounts, then reverses on scroll-to-top — no offset math.

- **footer copy not singularized** (i18n, LOW; agent 3). `Showing 1 of 1
  conversation` now singularizes on `total === 1`.

- **gallery demo reused a duplicate `data-testid`** (caught by RUNNING — the
  dev-server `testid-unique` plugin refused to boot). Removed
  `chat-history-pagination-card` from the demo footer (the visual e2e asserts on
  the visible text, and that id must stay unique to the real ConversationList).

## Rejected (not defects)

- **`scrollMargin` omitted** (plan-coverage, agent 4). Documented impl-wins
  (DRIFT-1.1): the virtual container sits at the viewport content top (M ≈ 0),
  matching MessageList which sets none. EMPIRICALLY validated — TEST-4/5 pass, so
  the window maps to the right rows at the right offsets. A `scrollMargin` here
  would be a no-op.

- **gallery drives `VirtualizedConversationList` directly, not full
  `ConversationList`** (plan-coverage, agent 4). Documented impl-wins (DRIFT-1.4),
  mirroring `MessageListLongDemo` (which drives MessageList directly, not the full
  ConversationPage). The demo DOES render the populated virtualized rows (the thing
  this feature changes); ConversationList's search/bulk chrome is unchanged by this
  diff and is a pre-existing gallery concern, not introduced here.

- **reactive-read-in-loop / security** (agent 4): both clean — `selectedIds`/
  `isSelectionMode`/`nativeScroll` are captured at the top level (no store proxy
  read inside `.map()`); no data/permission/exposure change.

## Re-audit

A full second blind round (fresh diff-only agent over the FIXED diff) found
**3 new confirmed findings** (all low): the estimator modeled the meta as
inline-only and under-estimated the STACKED narrow-width layout (< sm); and two
estimator unit tests used the LONG title which saturates the 2-line cap at both
widths, so their width-ordering assertions passed by equality. These are carried
into FIX_ROUND-2.

**New confirmed findings:** 3

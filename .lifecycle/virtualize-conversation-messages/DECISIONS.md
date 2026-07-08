# DECISIONS — virtualize-conversation-messages

Every input resolved by the existing dependency + the proven lazy-load
infrastructure on this branch. No product-level ambiguity → no AskUserQuestion.

### DEC-1: Which virtualization library?
**Resolution:** `@tanstack/react-virtual` (`useVirtualizer`).
**Basis:** codebase — already a dependency (`src-app/ui/package.json` `^3.14.4`)
and already used 3× (`kit/table.tsx`, `kit/tree.tsx`, `kit/multi-select.tsx`);
`table.tsx` already does dynamic variable-height virtualization with
`measureElement`. No new dependency, matches the repo idiom
([[feedback_match_existing_patterns]], [[feedback_check_library_before_custom]]).

### DEC-2: Where does the virtualizer live (component ownership)?
**Resolution:** `MessageList` owns the `useVirtualizer` + renders the rows;
`ConversationPage` provides the scroll element (`getScrollElement`, derived from
its existing `getViewport()`) and holds a `messageListRef` exposing
`scrollToMessageId`. The find bar receives a `scrollToMessage` callback from the
page.
**Basis:** codebase — preserves the existing split (page owns the scroll
container + sentinels + anchor lifecycle; list renders messages); mirrors how
`table.tsx` takes `getScrollElement` from its container.

### DEC-3: `estimateSize` + `overscan`?
**Resolution:** `estimateSize` = 140px (a typical short message row incl.
spacing); `overscan` = 8 rows.
**Basis:** convention — big enough overscan that a scroll-anchor's top-visible
row and near-viewport jump targets stay mounted, small enough to keep the DOM
light; `table.tsx` uses overscan 12 for short grid rows, 8 suits taller message
rows. `measureElement` corrects the estimate per row, so the exact value only
affects the pre-measure scroll estimate.

### DEC-4: Prepend scroll-anchor mechanism under virtualization?
**Resolution:** Index-based. Before `loadOlderMessages`, capture
`{ anchorId, viewportOffset }` for the top-most rendered message. After the
prepend renders, restore with the virtualizer:
`scrollToOffset(getOffsetForIndex(newIndexOfAnchor, 'start')[0] − viewportOffset)`,
and leave the virtualizer's `shouldAdjustScrollPositionOnItemSizeChange` (default
ON) to settle estimate→measured height corrections. Reuse the lazy-load observer
trigger + `pendingAnchorRef` lifecycle; only capture/restore becomes index-based.
**Basis:** convention — the standard TanStack reverse-infinite-scroll technique;
element-`getBoundingClientRect` anchoring can't be used because the anchor row
may be virtualized out immediately after the index shift. The scrollTop/
scrollHeight anchor e2e (TEST-5) is the objective check.

### DEC-5: Keep the existing sentinels + bottom-follow, or replace with index triggers?
**Resolution:** KEEP the top/bottom `IntersectionObserver` load-sentinels and the
`messagesEndRef` initial-jump + bottom-follow from lazy-load.
**Basis:** codebase — the virtualizer renders a `getTotalSize()` spacer so
scrollHeight is preserved; the sentinels (at content y=0 / bottom) and
`messagesEndRef` still fire/scroll correctly. Reusing them is the minimal,
lowest-risk change over inventing index-range triggers.

### DEC-6: Inter-row spacing (flex `gap-1` is lost under absolute positioning)?
**Resolution:** Move the per-row spacing into each measured row (vertical padding
on the row wrapper) so `measureElement` includes it; the virtual container itself
has no gap.
**Basis:** convention — `table.tsx` measures the whole row; spacing must be
inside the measured box or absolute rows abut.

### DEC-7: Initial bottom-jump when only estimates exist?
**Resolution:** Keep the existing pre-paint `scrollIntoView(messagesEndRef)`
jump; the `isAtBottom` follow re-scrolls to the true bottom as rows measure.
Additionally, when a jump target is requested, load THEN `scrollToIndex` (which
measures on the way).
**Basis:** convention — `messagesEndRef` is below the spacer so it always maps to
the (estimated, then measured) bottom; the follow settles it. No new machinery.

### DEC-8: Fate of the element-measurement `scrollAnchor.utils` path?
**Resolution:** Keep the existing pure fns (`pickTopAnchor`/`restoreDelta` +
their tests) — they remain valid + may serve the non-virtualized fallback — and
ADD the pure index-anchor math (`indexRestoreOffset`) the virtualized list uses.
**Basis:** codebase — don't delete tested, still-referenced helpers; extend.

### DEC-9: Streaming re-measures the last row every token — acceptable?
**Resolution:** Accept it. `measureElement` re-measures only the growing tail
row; the follow is gated to the true tail (`!hasMoreAfter` + `isAtBottom`) so it
tracks the growth without thrashing.
**Basis:** convention — single-row re-measure is cheap; matches how any measured
virtual list handles a growing row.

### DEC-10: Gallery — the virtualizer needs a real viewport height?
**Resolution:** Ensure the gallery MessageList surface renders inside a scroll
viewport with a fixed non-zero height so `useVirtualizer` measures a viewport and
mounts rows (a zero-height mock viewport would render nothing).
**Basis:** codebase — the component gallery must give the scroller a height; add
a state cell if the virtual/empty branch introduces a new conditional render.

### DEC-11: `scrollToMessageId` alignment for jump/find?
**Resolution:** `align: 'center'` (jump-to / find highlight), matching the
existing `scrollIntoView({ block: 'center' })` UX.
**Basis:** convention — same centering the lazy-load jump/find used.

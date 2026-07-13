# DECISIONS — chats-page-virtualization

All resolved up front so implementation runs nonstop. No `TBD`/`TODO`/`ASK`.

### DEC-1: Which virtualization library + measurement mode?
**Resolution:** `@tanstack/react-virtual` `useVirtualizer` with dynamic
`measureElement` + a content-aware `estimateSize` (ITEM-1). Identical to
`MessageList.tsx`.
**Basis:** codebase — the brief mandates reusing the MessageList precedent;
`@tanstack/react-virtual ^3.14.4` is already a `src-app/ui` dependency (used by
MessageList, kit/table, multi-select, tree).

### DEC-2: Generalize `measuredHeightCache.ts`, or add a conversation-scoped analog?
**Resolution:** Reuse `measuredHeightCache.ts` **as-is** — its API
(`getMeasuredHeight`/`setMeasuredHeight`/`recordMeasurements`/
`buildInitialMeasurementsCache`/`widthBucket`) is keyed by an opaque **string id**
+ width, with no message-specific logic. Conversation ids are UUIDs (disjoint
keyspace from message UUIDs; the module comment already notes cross-id reuse is
safe), so the new list imports the SAME functions. No fork, no duplicated LRU.
If a name reads message-specific, the module is renamed to a neutral
`measuredRowHeightCache.ts` with a re-export shim — but first choice is
import-as-is with zero edit to that file (keeps the message path byte-identical
and the live4/merge surface minimal).
**Basis:** convention — [[feedback_check_library_before_custom]] +
[[feedback_match_existing_patterns]]; the module is already id-generic.

### DEC-3: Desktop-only virtualization, or virtualize mobile too?
**Resolution:** Virtualize **desktop only** (inner OverlayScrollbars viewport);
mobile `nativeScroll` renders the (paging-bounded) loaded set **plainly**.
**Basis:** codebase — MessageList does exactly this: `@tanstack/react-virtual`
can't observe window scroll without a window-virtualizer, and the mobile set is
already bounded by paging. `virtualize = !nativeScroll`, keyed on the stable
`AppLayout.nativeScroll` flag.

### DEC-4: Overscan count?
**Resolution:** `overscan: 8`.
**Basis:** codebase — MessageList settled on 8 (its DEC-5 / FIX_ROUND-4 found a
drop to 4 regressed anchor precision). Conversation cards are cheaper than chat
messages, so 8 is safely conservative; `ConversationCard`-level memoization is
NOT assumed (see DEC-9).

### DEC-5: How is inter-row spacing rendered under absolute positioning?
**Resolution:** Vertical spacing lives **inside** each measured row (e.g. a
`pb-3`/`py-1.5` wrapper around the card), NOT as a parent flex `gap`. Horizontal
`px-3` gutter also moves inside the row.
**Basis:** codebase — MessageList DEC-6: a flex `gap` is lost when rows are
`position: absolute`; the current ConversationList relies on parent `gap-3`, which
must be internalized when virtualized.

### DEC-6: Handle the transient content above the virtual rows (bulk-actions bar / padding)?
**Resolution:** Set the virtualizer `scrollMargin` to the virtual container's
measured offset within the scroll viewport (via the container ref's
`offsetTop`), re-measured on `scrollerReady`/resize. The bulk-actions bar already
lives ABOVE the `DivScrollY` scroller (ConversationList:134, outside it), so in
practice the only in-scroller offset is the `py-3` padding — `scrollMargin`
covers it precisely so item offsets never skew.
**Basis:** codebase — `@tanstack/react-virtual`'s documented `scrollMargin` option
for content preceding the virtual list inside the scroll element.

### DEC-7: Keep the existing Load-More button, or switch to sentinel infinite-scroll?
**Resolution:** **Keep the existing Load-More button + "Showing N of M" footer**
unchanged, as a non-virtualized sibling below the virtual container. Do NOT add a
bottom sentinel or change the fetch trigger.
**Basis:** convention + coordination — this feature is *rendering-layer only*;
paging/scroll-trigger changes are **live4's** scope (sidebar infinite scroll).
Changing the fetch mechanism here would collide with live4 and exceed scope. The
pagination IDIOM for this top-level feed (Load-More) is already correct per the
UI-surface checklist.

### DEC-8: Any operational tunable → fixed constant or admin-configurable settings row?
**Resolution:** **Fixed constants**, no settings row. The only tunables are
`overscan` (8), the measured-height-cache `MAX_ENTRIES` cap (reused from the
existing module, 2000), and the estimator constants — all client-side rendering
micro-tuning with **no operator/security meaning**, exactly like MessageList's
equivalents (which are also fixed constants). Structured as named consts (not
inline magic numbers) so they remain tweakable.
**Basis:** convention — MessageList/`measuredHeightCache` ship these as fixed
consts; the Phase-4 configurable-settings rule's "security boundary / operator
concern" test does not apply to a client render-window size.

### DEC-9: Does the row renderer require `ConversationCard` to be memoized?
**Resolution:** Wrap the rendered card in `React.memo` at the row-render site (or
memoize the row) so scrolling doesn't re-render off-window rows unnecessarily —
but correctness must NOT depend on it. `ConversationCard` today is not memoized;
add a `memo` wrapper only at the virtualized call site to avoid editing the shared
card component (keeps the diff surgical + avoids affecting the project page /
recent-widget consumers).
**Basis:** convention — matches MessageList relying on memoized `ChatMessage`;
scoping the memo to the row site avoids touching a component used by other
surfaces.

### DEC-10: New scroller testid for the e2e?
**Resolution:** Add `data-testid` on the ConversationList card scroller (viewport
or its wrapper), e.g. `chat-conversation-list-scroll`, mirroring
`g-msglist-scroll`, so the behavioural e2e can drive/measure the scroll viewport.
Register it in the testid registry per `check:testid-registry`.
**Basis:** codebase — the scroll-stability e2e drives `g-msglist-scroll`; an e2e
must target a stable scroller handle ([[project_e2e_setup_submit_shadcn]] — prefer
testids over structural selectors for these).

### DEC-11: How does the behavioural "only a WINDOW is in the DOM" e2e get enough rows?
**Resolution:** Two complementary specs: (a) a **backend-free gallery** surface
seeding ~200 conversations (ITEM-7) → assert the mounted
`chat-conversation-card-*` count is far LESS than 200 (a bounded window), that
scrolling changes WHICH indices are mounted (top card detaches, a deep card
attaches), and that `__CHATLIST_METRICS__.corrections` settles to ~0 after a
pause (no jank); (b) a **real** seeded `/chats` spec (seed via
`POST /api/conversations`, Load-More to accumulate a large single set) → assert
the same window bound on the production path.
**Basis:** codebase — mirrors the MessageListLongDemo gallery + `conversation-
list-load-more.spec.ts` real-seed patterns. B7: the claim is proven by RUNNING it.

### DEC-12: Preserve keyboard accessibility / focus while virtualized?
**Resolution:** Rows keep `ConversationCard`'s `role="button"`, `tabIndex={0}`,
and `data-testid`; because off-window rows are unmounted, tab order only spans
mounted rows (standard virtualization tradeoff, identical to MessageList). No
`aria-rowcount`/grid semantics added (the list is not a grid today — no regression).
**Basis:** convention — parity with MessageList; a11y audit angle (Phase 6) will
confirm no NEW violation vs the current non-virtualized list.

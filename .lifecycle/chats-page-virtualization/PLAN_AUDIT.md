# PLAN_AUDIT — chats-page-virtualization

Audit of PLAN.md against the actual codebase, before writing code.

## Breakage risk

- **`ConversationList.tsx` inner-map swap (ITEM-5).** The only behavioural-risk
  edit. Consumers of `ConversationList` are `ChatHistoryPage` (the `/chats` page)
  and — grep-confirmed — the project detail page renders `ConversationCard`
  directly, NOT `ConversationList`, so the swap affects only `/chats`. The search
  portal, bulk-actions bar (rendered ABOVE the `DivScrollY` scroller at
  ConversationList:134 — confirmed OUTSIDE the scroll box, so it never enters the
  virtual-offset math), empty/error/loading arms, and store wiring are left
  byte-identical. Risk: the pagination footer ("Showing N of M" + Load-More)
  currently sits INSIDE the card map; moving it to a non-virtualized sibling below
  the virtual container must keep its `data-testid="chat-history-pagination-card"`
  + `chat-history-load-more-btn` + the aria-live status intact (existing
  `conversation-list-load-more.spec.ts` depends on them) — enumerated as TEST-9.
- **Nested scrollers.** `ChatHistoryPage` wraps `ConversationList` in a
  `DivScrollY nativeFlow` (outer) AND `ConversationList` has its own `DivScrollY`
  (inner) around the cards. The virtualizer attaches to the **inner** scroller
  (the one that actually overflows the cards). The outer is height-bounded by its
  `h-full` child so it does not scroll — no double-scroll regression. Confirmed by
  reading both files.
- **`measuredHeightCache.ts` reuse (ITEM-2, DEC-2).** Import-only; ZERO edit to
  the file → the message-scroll path is provably unaffected. No breakage surface.
- **DEV metrics (ITEM-6).** Behind `import.meta.env.DEV` → tree-shaken from prod,
  cannot affect production behaviour (parity with `__MSGLIST_METRICS__`).

## Pattern conformance

- **Virtualizer setup** mirrors `MessageList.tsx` (`useVirtualizer` with
  `count`/`getScrollElement`/`estimateSize`/`overscan`/`initialMeasurementsCache`/
  `onChange` coalesced write-back/`getItemKey`). Conforms.
- **`scrollMargin`** (DEC-6) is a **real** `@tanstack/virtual-core` v3.17.2 option
  (verified in `dist/esm/index.js:264`). Using it for the in-scroller offset is
  library-idiomatic (MessageList doesn't need it because its virtual container is
  effectively at the scroller top; ours has `py-3` + a possible sibling above).
- **Scroll-element wiring** mirrors `ConversationPage` (`osInstance().elements()
  .viewport`, `events={{ initialized: () => setScrollerReady(true) }}`,
  `getScrollElement={() => getViewport()?.root ?? null}`). Conforms.
- **Estimator** mirrors `estimateMessageHeight.ts` (cheap, total, memoized per
  (obj, width bucket) WeakMap). Conforms.
- **Gallery long-list demo + DEV-metrics e2e** mirrors `MessageListLongDemo.tsx`
  + `chat-scroll-stability.spec.ts` + the `seeded-*` surface registration in
  `seededSurfaces.tsx`. Conforms.
- **Store usage** — read reactive fields, call actions directly; no `useEffect`
  data-fetch added; no store mutation. Conforms to the store rules.

## Migration collisions

- **None.** No migration added (`ls migrations | tail -1` =
  `00000000000157_…`; this feature adds nothing). No collision with main or live4.

## OpenAPI regen

- **None.** No backend type/route/response change → no `openapi.json` /
  `api-client/types.ts` regen; not treated as backend work by the phase-3/8 gates.
  No new `/api/` route-mock is introduced by the specs (they hit real
  `POST /api/conversations` / the backend-free gallery), so R2-5 has no new surface.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — direct analog of `estimateMessageHeight.ts`; pure,
  no new dep, unit-testable in isolation.
- **ITEM-2** — verdict: PASS — `measuredHeightCache.ts` is already id-generic
  (opaque string key + width bucket; comment already blesses cross-id reuse);
  import-as-is, zero edit to the message path. DEC-2 resolved.
- **ITEM-3** — verdict: PASS — new component mirroring MessageList's virtualized
  branch; absolute rows + spacing-inside-row is the established DEC-6 pattern.
- **ITEM-4** — verdict: PASS — scroll wiring is a copy of the proven
  `ConversationPage` pattern; `scrollMargin` verified to exist. Desktop/mobile
  split keyed on the stable `nativeScroll` flag.
- **ITEM-5** — verdict: CONCERN — the only shared-file edit (live4 also edits
  `ConversationList.tsx`). Mitigation is in BASE.md: localized inner-map + ref
  swap, zero `ChatHistory.store.ts` edit, keep pagination testids. Not BLOCKED —
  the overlap is a single reconcilable region; whoever merges second fixes it.
- **ITEM-6** — verdict: PASS — DEV-only metrics, exact parity with the existing
  `__MSGLIST_METRICS__`; tree-shaken from prod.
- **ITEM-7** — verdict: CONCERN — the gallery/state-matrix/testid registries
  (`check:gallery-coverage`, `check:state-matrix`, `check:testid-registry`,
  `check:gallery-crawl`, `check:overlay-registry`) are strict; a new surface +
  testid must be registered in ALL required manifests or `npm run check` fails.
  Not BLOCKED — mechanical; budgeted in TEST-11 and the Phase-8 gate. Will follow
  the existing `seeded-chat-history-list` entry as the template.

**No BLOCKED verdicts.** Two CONCERNs (ITEM-5 merge overlap, ITEM-7 registry
strictness), both with concrete mitigations recorded — proceed.

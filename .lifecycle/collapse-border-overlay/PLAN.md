# PLAN — collapse-border-overlay (issue #183)

Thinking / tool-call card borders render washed-out when a long assistant message
is COLLAPSED ("Show more"); they are crisp when expanded. Defect is confined to
the clamped rendering state.

> **SUPERSEDED by `REPRO.md` (see DRIFT-1.1).** The root-cause section below was
> written from the task brief BEFORE the reproduction and is materially wrong: it
> attributes the dimming to the mask's alpha ramp. Measurement showed the ring is
> painted OUTSIDE the border box and that `overflow-hidden` and `mask-image` EACH
> independently clip it there — the ramp is a minor secondary effect. The ITEM
> list is unaffected; both items were still required. Kept unedited as the audit
> trail of what was believed at plan time.

## Root cause (two mechanisms)

Shared precondition: the kit `<Card size="sm">` border is **not** a `border-*`
property — it is `ring-1 ring-foreground/10`
(`sdk/packages/kit/src/shadcn/card.tsx:15`), a **10%-alpha box-shadow painted
OUTSIDE the padding box**. Both the Thinking card (`ThinkingContent.tsx:28`) and
the MCP tool-call cards (`mcp/chat-extension/extension.tsx:184`, `:332`) use it.

`CollapsibleBlock.tsx:117-118` applies, only when `isClamped`:

```
overflow-hidden [mask-image:linear-gradient(to_bottom,black_75%,transparent)]
```

with `maxHeight: 384` (`COLLAPSE_MAX_HEIGHT_PX`, `collapsible.ts:13`).

- **Mechanism A — `overflow-hidden` shaves the ring's left/right edges.** The
  parent clip layer (`ChatMessage.tsx:220-231`) carries `px-0.5` with an explicit
  comment that the 2px inset exists so "a full-width child Card's left/right
  border + rounded corners aren't shaved by the clip". The masked div
  re-establishes a clip **without** that inset. Position-independent — hits cards
  anywhere in the clamp.
- **Mechanism B — the fade ramp erases rings below 288px.** The ramp starts at 75%
  of 384px ≈ 288px. A collapsed Thinking card (~44px + `mb-2`) plus two or three
  collapsed tool cards (~44px each) readily lands mid-ramp, where a `0.10` ring is
  multiplied to ~`0.05` then 0, while `bg-card` and label text stay legible.

Both are gated on `isClamped`, which is why expanding restores both.
The top 75% of the mask is alpha exactly `1.0`, so B **cannot** dim anything above
288px — a mask-only fix would leave A untouched.

**Why it shipped:** no gallery fixture renders the buggy combination. The only
collapsible fixture (`MessageListLongDemo.tsx:96-97`, `i % 7 === 0`) is pure long
TEXT — no thinking, no tool block. `coverage.ts:240,550` mark both
`ThinkingContent` and `CollapsibleBlock` as `via`.

## Items

- **ITEM-1**: Add a gallery deep-state surface `deep-chat-collapsed-tool-boxes` —
  ONE assistant message carrying a thinking block + a tool_use/tool_result pair +
  >1200 chars of trailing text, so it clamps with cards inside the clamp. This is
  both the reproduction and the permanent regression pin. Include an `interactions`
  recipe that expands the collapsible so the expanded state is captured too.
- **ITEM-2**: [DESCOPED] The `splitTrailingProse` / `classifyNode` helpers — the
  split they served is reverted (see ITEM-3), so they have no consumer.
- **ITEM-3**: [DESCOPED] Splitting the clamp so structural cards render above the
  fold. Implemented, then REVERTED after the blind audit proved it causes two HIGH
  regressions (a long turn ending on a tool card loses height-bounding entirely —
  measured at 1044px with no toggle). `ChatMessage.tsx` is unchanged by this diff.
  See DEC-1 (revised) and DRIFT-2.1.
- **ITEM-4**: **THE FIX.** Add a `-mx-0.5 px-0.5` inset to the clamped container in
  `CollapsibleBlock.tsx`. Equal negative margin + padding leaves the CONTENT box
  byte-identical while pushing the element's box edge outward, giving a child
  Card's `ring-1` (a 1px-spread box-shadow painted OUTSIDE its own box) room
  inside BOTH the `overflow-hidden` clip and the `mask-image` painting area
  (`mask-clip` defaults to `border-box`). Unconditional, not gated on `isClamped`,
  so toggling causes no reflow. Verified by reproduction: rings 0 → 25 (light) /
  0 → 23 (dark) with the mask and clip still active, card width unchanged.
- **ITEM-5**: Update the `coverage.ts` entries for `CollapsibleBlock` /
  `ThinkingContent` to name the new surface as what pins their clamped state,
  instead of the generic `via` reason. *(Amended in phase 5 — DRIFT-1.8. Originally
  also "regenerate the gallery registries"; that proved unnecessary AND harmful:
  this diff adds no component file and no `data-testid`, so the generators emit
  nothing new for it, and running them instead absorbed ~1700 lines of unrelated
  pre-existing SDK-migration drift into the diff.)*

## Files to touch

- `src-app/ui/src/modules/chat/components/collapsible.ts` (ITEM-2)
- `src-app/ui/src/modules/chat/components/collapsible.test.ts` (ITEM-2 tests)
- `src-app/ui/src/modules/chat/components/ChatMessage.tsx` (ITEM-3)
- `src-app/ui/src/modules/chat/components/CollapsibleBlock.tsx` (ITEM-4 — the
  ONLY change to this file; the mask expression itself is untouched)
- `src-app/ui/src/modules/chat/gallery.tsx` (ITEM-1)
- `src-app/ui/src/dev/gallery/fixtures/chat-deep.ts` (ITEM-1 fixture bundle)
- `src-app/ui/src/dev/gallery/coverage.ts` (ITEM-5)
- generated registries (ITEM-5): `galleryCoverage.generated.ts`,
  `stateMatrix.generated.ts`, seed/testid registries — via `npm run gen:*`

## Patterns to follow

- **Gallery deep-state** — mirror the existing `deep-chat-tool-group` entry in
  `chat/gallery.tsx` (~line 194) and its bundle in
  `src/dev/gallery/fixtures/chat-deep.ts` (`message()` / `block()` builders,
  registered in `chatDeepById` + `CHAT_DEEP_CONVERSATION_IDS`). Use the same
  `interactions: [{ name, note, steps }]` shape as
  `deep-chat-mcp-toolcall-error` (which drives an expand button) rather than
  hand-authoring a bespoke visual spec.
- **Pure helper + node:test** — mirror `shouldOfferCollapse` in `collapsible.ts`
  and its existing `collapsible.test.ts` (`node:test` + `assert/strict`), run via
  `npm run test:unit`.
- **Not Vitest** — chat helpers use `node:test`; Vitest is scoped to
  `src/**/*.store.test.ts` only.
- **No React Testing Library exists for chat** — render behavior is proven through
  the gallery + Playwright layers (Layer A picks a new surface up automatically
  from `sectionTestIds`), not a component unit test.

## UI-surface checklist

This is a bugfix to an EXISTING surface plus a dev-only gallery fixture; no new
product surface, no new permission, no new data fetching.

- **Precedent** — the new gallery entry is the twin of `deep-chat-tool-group`;
  the fixed message rendering must stay visually identical to today's EXPANDED
  state (which is the correct rendering) in both themes.
- **Scale / cardinality** — unchanged. No list, no fetch. One seeded conversation.
- **Device size / responsive** — the clamp is a fixed 384px height at every
  breakpoint. Layer A already runs 390/768/1280, so the new surface is exercised
  at narrow width for free; verify the cards' rings are crisp at 390px too, where
  a full-width card sits hardest against the clip edge (Mechanism A is most
  visible there).
- **Populated-render review** — the surface is populated by construction (that is
  its whole point); the design-critic pass reviews the POPULATED collapsed render
  at each viewport × theme.
- **User-visible progress / input economy / multi-instance / URL-as-view /
  platform affordances** — not applicable; no new interaction, no new state, no
  new route.

## Risks

- **Virtualizer height estimates** — `estimateMessageHeight.ts:87-90` gives
  tool/thinking blocks a flat `TOOL_ADD`; moving them out of the clamp changes
  real row heights. `chat-scroll-stability` TEST-6 (corrections ≤2 when settled)
  is the detector.
- **A turn that is mostly tool cards** is no longer height-bounded by the clamp.
  Acceptable because the cards are collapsed by default (`ThinkingContent.tsx:16`,
  `extension.tsx:184/332`) and carry their own toggles. Revisit only if the
  reproduction shows a pathological case.

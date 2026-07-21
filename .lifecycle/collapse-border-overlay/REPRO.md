# REPRO — collapse-border-overlay (issue #183)

Measured against the new `deep-chat-collapsed-tool-boxes` gallery surface at
1280×900, `deviceScaleFactor: 1`, light theme, BEFORE any fix code was written.
Method: Playwright geometry probe + 1px-wide screenshot strips straddling each
card edge. Ring visibility is reported as `maxDelta` — the largest per-channel
deviation across the strip. The kit ring renders `rgb(230,230,230)` on a white
background, so **delta 25 = ring visible, delta 0 = ring absent**.

## The corrected root cause

The task brief (and my own Phase-1 plan) attributed the dimming to the mask's
**alpha ramp**. The measurements refute that. The actual mechanism:

1. The kit `<Card>` border is `ring-1 ring-foreground/10`
   (`sdk/packages/kit/src/shadcn/card.tsx:15`). Computed, that is a box-shadow
   with **1px SPREAD and zero offset/blur** — `oklab(0.145 0 0 / 0.1) 0px 0px 0px
   1px` — i.e. painted **entirely OUTSIDE the border box**.
2. The card is **flush with its container**: measured `leftInset: 0`,
   `rightInset: 0`. So 100% of that ring lies outside the container's box.
3. When clamped, `CollapsibleBlock` applies **both** `overflow-hidden` **and**
   `mask-image`. Each clips to the border box independently — `mask-clip`
   defaults to `border-box`, so the mask's painting area excludes anything drawn
   outside the box, regardless of the gradient's alpha.

So the ring is clipped away, everywhere, at any vertical position. The alpha ramp
is a secondary, minor effect.

## Evidence 1 — the defect, and that it is confined to the collapsed state

| card | topRel→bottomRel | collapsed LEFT | collapsed TOP | expanded LEFT | expanded TOP |
|---|---|---|---|---|---|
| `thinking-card` | 0 → 52 | **0** | **0** | 25 | 25 |
| `mcp-tooluse-card-toolu_collapsed_1` | 108 → 160 | **0** | 25 | 25 | 25 |
| `mcp-tooluse-card-toolu_collapsed_2` | 264 → 316 | **0** | 25 | 25 | 25 |

Every LEFT ring is gone while collapsed and returns on expand. The
`thinking-card` additionally loses its TOP ring because it sits flush at
`topRelClamp: 0`, so its top ring is outside the box too. The surviving
horizontal segments on the other two are why the boxes read as "washed out /
broken" rather than simply absent.

Clamp state at measurement: `height: 384`, `data-collapsed: "true"`,
`overflow: hidden`, `mask-image: linear-gradient(rgb(0,0,0) 75%, rgba(0,0,0,0))`,
`padding-inline: 0px/0px`, ramp starts at **288px**. Zero console errors.

## Evidence 2 — isolation: each cause is INDEPENDENTLY sufficient

Held collapsed; toggled ONE property at a time. Values are LEFT ring delta.

| condition | thinking | tool 1 | tool 2 | reading |
|---|---|---|---|---|
| baseline (mask + clip) | 0 | 0 | 0 | ring absent |
| **mask removed only** | 0 | 0 | 0 | clip alone still removes it |
| **clip removed only** | 0 | 0 | 0 | mask alone still removes it |
| both removed (= expanded) | 25 | 25 | 25 | ring returns |

This is the load-bearing result. Neither cause alone explains the defect and
neither alone fixes it — **`overflow-hidden` and `mask-image` each independently
clip the outside-painted ring**. A fix that only removed the mask, or only
relaxed the ramp, would have measured as no improvement at all.

## Evidence 3 — the fix hypothesis, verified before writing it

Held collapsed (mask + clip both active) and applied DEC-5's
`padding-inline: 2px` + `margin-inline: -2px`:

| card | leftRingDelta before → after | leftInset before → after | cardWidth before → after |
|---|---|---|---|
| `thinking-card` | 0 → **25** | 0 → 2 | 860 → **860** |
| `…toolu_collapsed_1` | 0 → **25** | 0 → 2 | 860 → **860** |
| `…toolu_collapsed_2` | 0 → **24** | 0 → 2 | 860 → **860** |

- Rings fully restored while still clamped — the inset gives the ring room inside
  **both** the overflow clip and the mask painting area.
- `cardWidth` is byte-identical at 860px, confirming DEC-5's claim that equal
  negative-margin + padding leaves the content box untouched (no reflow on
  toggle). This is why a bare `px-0.5` was rejected.
- Card 2 reads **24, not 25** — it spans 264→316 and its mid-height sits right at
  the 288px ramp start, so the alpha ramp attenuates it slightly. This is
  Mechanism B, observed directly, and it is exactly what ITEM-3's split removes by
  taking cards out of the masked region altogether.

## Evidence 4 — AFTER the fix (same probe, same surface)

Ring deltas with the cards hoisted out of the clamp AND the inset applied. Both
themes, both states:

| theme | state | card | leftRing | topRing | insideClamp |
|---|---|---|---|---|---|
| light | collapsed | all three | **25** | **25** | false |
| light | expanded | all three | **25** | **25** | false |
| dark | collapsed | all three | **23** | **23** | false |
| dark | expanded | all three | **23** | **23** | false |

Collapsed and expanded are now **identical** — which is the real success
criterion, since expanded was always the correct rendering. Light reads 25 and
dark 23 because `ring-foreground/10` resolves against a different foreground per
theme; both are the full, unclipped value for their theme.

Clamp still behaves: `height: 384`, `data-collapsed: "true"`, mask present,
`padding-inline: 2px/2px`, `margin-inline: -2px/-2px`. Zero console errors.

Visual confirmation (screenshots, light theme, cards scrolled into view):
- **Before** — the Thinking and both `query_rag` cards render with NO left or
  right border; only the horizontal segments survive, so each box reads as
  open-sided. This is the reported "washed out" appearance.
- **After** — all four sides close on every card, rounded corners intact, block
  order `thinking → text → tool → text → tool → answer` preserved, and the prose
  still fades at the fold with the Show more affordance.

### A fixture trap worth recording

Sizing the fixture against the WHOLE turn was wrong. Once the cards are hoisted
above the fold only the trailing prose is measured, and at the original length it
came to **368px — under the 384px clamp**, so the message silently stopped
collapsing (`data-collapsed: null`, `mask-image: none`) and the surface stopped
covering the bug at all. The prose block must exceed the clamp ON ITS OWN. TEST-2
now asserts `data-collapsed === 'true'` precisely so this cannot regress
unnoticed.

### The specs were verified to FAIL without the fix

All 6 specs in `chat-collapse-borders.spec.ts` were re-run with
`ChatMessage.tsx` + `CollapsibleBlock.tsx` reverted: **6 failed**, reporting
`Expected length: 0 / Received length: 3` with the three cards at
`insideClamp: true`. A regression test that passes on the unfixed code proves
nothing; these do not.

## What this changes

- **ITEM-4 is promoted from defensive polish to the PRIMARY fix.** It is what
  restores the rings.
- **ITEM-3 (the split) remains correct and is retained** — it eliminates the
  residual ramp attenuation shown on card 2 and enforces the principle that
  structural cards are not prose to be faded. It is no longer the sole fix, so it
  is worth noting for review that ITEM-4 alone would resolve the user-visible
  defect; ITEM-3 is retained per the explicitly approved DEC-1. Surfaced rather
  than silently reversed, per the audit-vs-user-decision rule.
- The Phase-1 root-cause narrative (ramp-driven) is superseded by this file.
  PLAN.md is amended in the DRIFT round accordingly.

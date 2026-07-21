# FIX_ROUND-2 — collapse-border-overlay

A full blind re-audit against the reverted (inset-only) diff. It found four HIGH
issues — one a real remaining product defect, three about the tests claiming more
than they proved. All fixed and re-verified.

## Product defect

**The inset was horizontal-only, so the FIRST card's top ring was still clipped.**
The first child card's border-box top is flush with the clamp's top, and the clip
applies on all four sides. Measured: `thinking-card` top ring delta **0 collapsed
vs 25 expanded** — the same issue-183 symptom, simply unfixed on the top edge. My
own very first probe had recorded this and I had not carried it forward once the
split (which moved cards out of the clamp entirely) masked it.

Fix: widen the inset to all four sides (`-m-0.5 p-0.5`). The vertical overhang is
safe because the ancestor deliberately uses `overflow-x: clip` rather than
`hidden` specifically so `overflow-y` stays truly visible — the sibling comment in
`ChatMessage` documents that exact reasoning for this exact bug class. Re-measured:
every card now reads 25 (light) / 23 (dark) on left and top in BOTH states.

## Test-honesty defects

- **A false claim in the code comment.** It asserted the spec would catch a parent
  that stopped absorbing the inset's overhang. It would not: `expectRingRoom`
  measured against the clamp's own rect, never the clipping ancestor. Fixed on
  both sides — `readTurn` now walks to the nearest clipping ancestor (first
  ancestor with a non-visible overflow, or a mask) and measures against it, and
  the comment no longer promises protection it cannot deliver.
- **The suite never checked that a ring is PAINTED.** Pure geometry: a kit Card
  changed to `ring-0` would erase every border in every state with TEST-3 — named
  "every card's ring renders" — still green. Added `isEdgePainted()`, which
  screenshots a 2px strip covering the ring pixel plus 1px of background and
  compares it byte-wise against an identical strip of bare background. No
  image-decode dependency: two PNGs of identical pixel data encode identically.
- **`expectRingRoom` checked only left/right**, so the top-edge defect above was
  structurally invisible to the suite. Now all four sides.

## Self-found while validating the paint check

The first `isEdgePainted` was wrong twice, and only running it against
deliberately-broken code exposed that:
1. It added `window.scrollX/Y` to the screenshot clip. Playwright's `clip` for a
   viewport screenshot is already in viewport coordinates, so every border read as
   missing — including on correct code.
2. After fixing that, its 5px window spanned the card's INTERIOR. In dark mode
   `bg-card` differs from the page background, so the strip differed regardless of
   the ring: a `ring-0` regression **passed in dark while failing in light**.
   Narrowed to exactly the ring pixel + 1px of background.

This is why each assertion was validated against a broken build rather than just a
green one.

## Verification of the fixes

| check | result |
|---|---|
| horizontal-only inset (the round-2 defect) | TEST-3 FAILS: "only 0px … on TOP — its 1px ring is clipped there" |
| kit Card set to `ring-0` | TEST-3 + TEST-8 FAIL in BOTH themes |
| correct code | 7/7 collapse specs pass, both themes |
| regression guards | 7/7 chat-scroll-stability pass |
| backend e2e | collapse-long-message passes |

## Accepted, not fixed

The mask ramp still attenuates a ring in the 288–384px band (~4% at the ramp
start). That is the intended "more below" cue and it fades all content there, not
just rings; removing it would reintroduce the color band the mask exists to avoid.
The reported defect was the hard clip, which erased rings entirely at any
position.

**New confirmed findings:** 0

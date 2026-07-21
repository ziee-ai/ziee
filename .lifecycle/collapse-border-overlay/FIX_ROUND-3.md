# FIX_ROUND-3 — collapse-border-overlay

Full blind re-audit against the four-sided-inset diff.

## The fix itself: no defects found

The auditor verified the DOM/CSS chain independently rather than trusting the
comments, and confirmed: the horizontal half lands the content div's border box
exactly on the bubble's `overflow-x: clip` edge with cards 2px inside; `overflow:
hidden` clips at the padding box, so the vertical padding genuinely buys ring
room; `overflow-x: clip` really does leave `overflow-y` visible, so the 2px top
overhang is not re-clipped; the `w-fit` user bubble is unaffected (margin −4 and
padding +4 cancel in the max-content contribution); RTL is symmetric; virtualizer
measurement is unchanged.

Every finding was in the TEST FILE or in a COMMENT that overstated what the tests
prove.

## Fixed

| # | issue | resolution |
|---|---|---|
| 1 | TEST-4 could not detect a state-gated inset — and the "would reflow" rationale behind it was itself false, since the inset self-cancels either way | TEST-4 now asserts the inset SELF-CANCELS (clamp content width == parent content width), catching `p-0.5` written without `-m-0.5`; the component comment drops the false rationale |
| 2 | `roomBottom >= 1` was over-strict — the clamp's bottom IS the fold, so ordinary fixture drift would raise a false "#183" alarm | bottom removed from the assertion set, reasoning recorded inline |
| 3 | `isEdgePainted` failed OPEN on unmeasurable coordinates, silently vacating the only check covering the vertical half | now throws, naming the card and its position |
| 4 | the ring-room doc comment claimed it caught removal of the parent's `px-0.5`; it did not — the nearest clipper while collapsed is the clamped container itself | room is now the TIGHTEST gap across EVERY clipper in the message, tracked PER AXIS; verified by deleting `px-0.5` → TEST-3 fails "only 0px … on LEFT" |
| 5 | "content box byte-identical" was true for width but not the clamp (border-box + 4px vertical padding ⇒ 380px visible, not 384) | comment corrected, with the consequence of scaling the inset up spelled out |
| 6 | the toggle's gap dropped to 2px (the `-mb-0.5` is not collapsed in a flex column), off the 4px rhythm | already fixed independently before this round: `mt-1.5`, coupling documented; re-measured at 4px |

## Self-found while fixing #4

Two wrong attempts, each caught by running rather than reasoning:
- walking past the message root picked up the message-list SCROLL container, whose
  clipping is a scroll boundary — a card near the viewport top legitimately has
  ~0px room against it;
- taking the tightest gap across all clippers without distinguishing axes reported
  `roomTop: 0` against the bubble's clip layer, which is `overflow-x: clip` with
  `overflow-y: visible` and does not clip vertically at all.

Diagnosed by dumping the actual clipper chain instead of guessing a third time.

## Assertion validation (each observed to FAIL on the defect it claims to catch)

| injected defect | result |
|---|---|
| unfixed base (no inset) | TEST-3 fails — LEFT |
| horizontal-only inset | TEST-3 fails — TOP |
| kit Card set to `ring-0` | TEST-3 + TEST-8 fail in BOTH themes |
| parent `px-0.5` deleted | TEST-3 fails — LEFT, against the bubble clip layer |
| correct code | 7/7 collapse + 7/7 scroll-stability pass |

**New confirmed findings:** 0

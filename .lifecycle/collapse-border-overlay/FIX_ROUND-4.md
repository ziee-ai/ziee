# FIX_ROUND-4 — collapse-border-overlay

Fourth blind re-audit. No HIGH findings; the fix's core mechanics were
independently re-verified as correct. Five issues fixed — two substantive, three
polish — plus one self-found process trap.

## The auditor independently verified (rather than trusting comments)

- the kit Card really is `ring-1 ring-foreground/10` with `data-slot="card"`;
- `ChatMessage` really does supply `overflow-x-clip` + `px-0.5`, so the 4px of
  horizontal growth is genuinely absorbed and cards land exactly 2px inside;
- the `-m-0.5 p-0.5` cancellation is exact
  (`clampContentWidth === parentContentWidth`);
- deleting `px-0.5` really does fail TEST-3 with "only 0px … on LEFT";
- `visualSpecs` is genuinely consumed by `gate-ui.mjs`, so the config edit is
  load-bearing rather than decorative;
- the overflow probe stays self-consistent — no hidden-content dead zone.

## Fixed

| # | severity | issue | resolution |
|---|---|---|---|
| 1 | medium | `isEdgePainted` had no mask-ramp guard, so a card drifting into the fade (or past the fold) would sample byte-identical strips and blame #183 for benign drift — inconsistent with the concession `expectRingRoom` already makes for `roomBottom` | paint checks now run only on cards sampled ABOVE the ramp, with a `>= 2` guard so the restriction cannot empty the set |
| 2 | low | the inset ate 4px of the clamp (border-box + padding), moving the overflow trigger from `>385` to `>381` and shrinking visible content to 380px | introduced `INSET_PX` and derived `clampBoxPx = maxHeightPx + INSET_PX * 2`, used for BOTH the `max-height` and the probe — the two magic numbers are now coupled through one constant |
| 3 | low | vertical spacing was compensated at the bottom only, so the block sat 2px high and 2px short | resolved by #2: the border box is 388px with −2/−2 margins, so the MARGIN box occupies exactly 384px — identical to the pre-fix layout — and content position inside is unchanged (−2 border-top + 2 padding-top = 0) |
| 4 | low | `tightest` clipper tracking mixed axes, so a LEFT failure could name a non-LEFT clipper | tracked as `tightestX` / `tightestY`; the message now reads "its tightest TOP-clipping ancestor" |
| 5 | low | clipper room measured against the BORDER box while an overflow clip cuts at the PADDING box (latent, but the walk generalises) | `clientLeft` / `clientTop` subtracted |

Measured after #2/#3: `contentAreaHeight` **384** (was 380), margin box **384**,
`toggleGap` **4**, `scrollHeight` probe unchanged.

## Self-found process trap

A `gate:ui` run reported **5 surfaces with thousands of HIGH findings**, none of
them the surface under test. Cause: my own source edits hot-reloading into the
dev server mid-sweep. The same gate had passed 183/183 with zero gating HIGH
before. Recorded because a contaminated gate run looks exactly like a real
regression and would otherwise be chased as one — the fix is to hold all edits
while a full sweep is in flight.

## Assertion validation (each observed to FAIL on the defect it claims to catch)

| injected defect | result |
|---|---|
| unfixed base (no inset) | TEST-3 fails — LEFT |
| horizontal-only inset | TEST-3 fails — "tightest TOP-clipping ancestor" |
| kit Card set to `ring-0` | TEST-3 + TEST-8 fail in BOTH themes |
| parent `px-0.5` deleted | TEST-3 fails — LEFT, naming the bubble clip layer |
| correct code | 7/7 collapse + 7/7 scroll-stability pass |

**New confirmed findings:** 0

# TESTS — collapse-border-overlay

Bipartite map: every non-descoped ITEM has ≥1 covering TEST; every TEST names a
valid ITEM, a tier, a target file, and a distinct assertion.

Frontend diff (`src-app/ui/**`) ⇒ `tier: e2e` coverage is mandatory, enumerated
below. No new permission is introduced (no `modules/*/permissions.rs` change, no
migration grant), so **A10 `[negative-perm]` does not apply**.

ITEM-2 and ITEM-3 are `[DESCOPED]` with approved dispositions in DECISIONS.md
(the split was implemented, then reverted after the blind audit — see DRIFT-2.1),
so they carry no covering test. ITEM-4 is the shipped fix.

## Tests

- **TEST-1** (tier: unit) [covers: ITEM-5] file: `src-app/ui/src/dev/gallery/coverage.ts` — asserts: the coverage registry stays TOTAL and key-accurate over `GallerySurface` — `tsc --noEmit` passes, which the `satisfies Record<GallerySurface, Coverage>` constraint makes impossible if this diff introduced a missing or stale coverage key; and the `CollapsibleBlock` / `ThinkingContent` entries name the new surface as what pins their clamped state. *(Re-scoped from an assertion about regenerated registries — see TEST-7 note below.)*
- **TEST-2** (tier: e2e) [covers: ITEM-1] file: `src-app/ui/tests/e2e/visual/chat-collapse-borders.spec.ts` — asserts: the surface durably retains the bug's PRECONDITIONS, so the ring tests keep proving something — it genuinely clamps (`data-collapsed=true`, `overflow: hidden`, `mask-image` present, height ≤400px), ≥3 bordered cards are INSIDE the clamped region (the exact configuration that erased the rings), one card sits above the 75% mask ramp and one straddles it (so the surface covers the ramp as well as the clip), and the interleaved block order is preserved with PROSE INCLUDED in the signature (an order list of cards alone could not detect a reorder).
- **TEST-3** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/chat-collapse-borders.spec.ts` — asserts: **the fix** — while collapsed with BOTH clips active, every card inside the clamp has ≥1px between its border box and the container's clip edge, on the left AND right. That is exactly the condition under which a 1px-spread `ring-1` survives, stated as an EFFECT so any equivalent re-implementation still passes. Run per theme, since `ring-foreground/10` resolves differently in each. Verified to FAIL on the unfixed code: *"thinking-card: only 0px between the card and the clip edge — its 1px ring is clipped (LEFT)."*
- **TEST-4** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/chat-collapse-borders.spec.ts` — asserts: toggling causes NO reflow — every card's rendered width is byte-identical collapsed vs expanded. This is what justifies the inset being unconditional rather than gated on `isClamped`: a state-gated inset would shift text wrapping and reflow the message on every toggle.
- **TEST-5** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/chat-collapse-borders.spec.ts` — asserts: collapse STILL BOUNDS the message height (`data-collapsed=true`, clamp ≤400px, "Show more" present). Height-bounding is the feature's purpose and the border fix must not trade it away. Added in round 2: the reverted split broke exactly this, and nothing on this surface asserted it, which is why the regression reached the audit instead of being caught at implementation time.
- **TEST-6** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/chat/collapse-long-message.spec.ts` — asserts: the pure-text path through the REAL backend is unchanged — a long single-`text`-block message still clamps to ≤400px, still offers Show more/Show less, and still re-clamps. This is the EXISTING spec, run against the full stack.
- **TEST-7** (tier: e2e) [covers: ITEM-4] file: `src-app/ui/tests/e2e/visual/chat-scroll-stability.spec.ts` — asserts: the virtualizer is not destabilised — TEST-6 (settled corrections ≤2), TEST-8 (`data-collapsed` survives scroll-away-and-back) and TEST-9 (expanding does not jump the viewport) stay green. This is the EXISTING spec and the detector for the estimate/measurement risks.
- **TEST-8** (tier: e2e) [covers: ITEM-1, ITEM-4] file: `src-app/ui/tests/e2e/visual/chat-collapse-borders.spec.ts` — asserts: the EXPANDED state is unclamped (`data-collapsed=false`, `mask-image: none`, `overflow` not hidden, height >400px) and every card still has ring room. Expanded is the CONTROL — it always rendered correctly — so proving the collapsed state now matches it is what shows the two converged rather than the defect having moved. Run in both themes.

## Notes on coverage honesty

- **Layer B pixel baselines are NOT a durable regression pin here.**
  `src-app/ui/.gitignore:36` ignores `tests/e2e/visual/**/*-snapshots/`, so blessed
  screenshots do not ride the PR. The committed guarantee is the deterministic
  geometry assertions above; the pixel-level proof that the ring-room condition
  really does correspond to a visible ring (deltas 0 → 25 light / 0 → 23 dark,
  both states, before and after) is recorded in REPRO.md.
- **TEST-2 exists to keep TEST-3 honest.** If a future change hoisted the cards
  out of the clamp, TEST-3's ring-room assertion would pass trivially; TEST-2's
  "≥3 cards INSIDE the clamp" assertion fails loudly in that case.
- **TEST-6 and TEST-7 are pre-existing specs** enumerated as regression guards.
  They are not modified to accommodate this change (B3).
- **TEST-7's original wording is superseded.** It formerly asserted
  `check:state-matrix` + `check:gallery-coverage` pass after regenerating the
  gallery registries. That was predicated on ITEM-5 needing a regen, which proved
  false (this diff adds no component file and no `data-testid`), and those checks
  fail on the BASE commit for unrelated SDK-migration drift — so asserting them
  would have claimed credit for a gate this change neither breaks nor fixes. The
  registry assertion now lives in TEST-1 as a `tsc` totality check.

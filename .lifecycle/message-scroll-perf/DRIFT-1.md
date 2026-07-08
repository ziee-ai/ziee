# message-scroll-perf — DRIFT-1 (implementation vs plan)

Audit of the implemented diff against PLAN.md / TESTS.md / DECISIONS.md.

- **DRIFT-1.1** — verdict: impl-wins — **Unit tier is `node:test` pure-logic, not
  RTL component render.** TESTS.md (phase 3) enumerated `.test.tsx`
  React-Testing-Library render tests for `ReservedImage` / the `img` override /
  `MarkdownTable`. The repo has NO component-render unit harness (unit = Node's
  built-in runner, `*.test.ts`, pure logic; component behavior is covered by
  e2e). Resolution: the render logic was refactored to expose PURE helpers
  (`reservedImageBox`, `classifyImageSrc`) that are unit-tested, and render
  behavior moved to e2e (TEST-9). PLAN.md *Files to touch* + TESTS.md amended;
  phases 1–3 re-gated. Plan was wrong about the harness; impl matches repo
  convention ([[feedback_match_existing_patterns]]).

- **DRIFT-1.2** — verdict: impl-wins — **ITEM-4 required no code change.** The plan
  hedged that a definite-height bound might need adding to `MarkdownTable` /
  inline previews. Verification found both already bounded
  (`MarkdownTable` → `max-h-[min(60vh,36rem)]`; `InlineFilePreview` →
  `max-h-[min(360px,55vh)]` / `max-h-[600px]`; inline CSV already fixed). ITEM-4
  ships as verification only, covered by TEST-11 (table cap) + the existing
  `inline-csv-height-stability` spec. `MarkdownTable.tsx` is NOT in the diff.

- **DRIFT-1.3** — verdict: resolved — **Incidental security hardening in
  `classifyImageSrc`.** Extracting the `img` override's inline policy into a pure,
  tested classifier surfaced a latent hole: the original `src.startsWith('/')`
  auto-allow also matched a protocol-relative `//evil.test/x` (which resolves to a
  DIFFERENT origin) → an external image would render (exfil beacon). The extracted
  classifier adds `&& !src.startsWith('//')` so protocol-relative URLs go through
  the origin check and are blocked. This STRENGTHENS DEC-3 ("do not weaken the
  image security policy") rather than violating it; asserted by TEST-4. Confined
  to the same code ITEM-3 already touches; no scope expansion.

- **DRIFT-1.4** — verdict: impl-wins — **ITEM-6 kept the before-paint restore +
  added an idempotency guard (no rAF deferral).** DEC-6 floated deferring the
  restore to a post-measure `requestAnimationFrame`. That would paint an
  un-restored frame first (a visible flash), which is worse. Implementation keeps
  the synchronous `useLayoutEffect` restore and adds only `anchorRestoreNeeded`
  (skip a redundant `scrollToOffset` when the virtualizer's own above-viewport
  adjustment already pinned the anchor within 2px) — idempotent, no flash, no
  regression to the no-teleport invariant (TEST-5 unit + TEST-10 e2e). DECISIONS
  DEC-6's "may collapse to guard" branch is the realized one; the guard is the
  code change, not a no-op.

- **DRIFT-1.5** — verdict: none — estimator (ITEM-1), measured-height cache
  (ITEM-2), overscan 8→4 (ITEM-5), and the `ReservedImage` routing (ITEM-3)
  landed exactly as planned/decided (DEC-1/2/4/5). `onChange(sync=false)`
  write-back + unmount flush match DEC-2's "settled-only, no per-frame O(n)"
  requirement (verified: virtual-core calls `notify(false)` on measurement,
  `notify(isScrolling)` on scroll).

**Unresolved drifts:** 0

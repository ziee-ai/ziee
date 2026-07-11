# HUMAN_FEEDBACK.md — living ledger

This feature is **paused at Phase 4 for design approval** before any
implementation (per the assignment: "send me the design for approval — we will
ITERATE on the design conversationally"). The ledger opens here and records every
piece of human feedback verbatim as the design is iterated.

Status: design approved; Phase 5 implementation in progress.

- **FB-1** [status: resolved] — "Go more aggressive" (mechanism) + "include .desktop.tsx co-location, two exemplar conversions, and bulk-migrate existing overrides" (scope) → design expanded to the aggressive path: ts-morph auto-migration codemod + `.desktop.tsx` co-location in the core tree; triage split the 18 shadows 5B/5A/8C and the plan converts the 5 class-B to seams, relocates the 5 class-A, leaves the 8 infra. [generalizable: no — feature-specific scope call]
- **FB-2** [status: resolved] — "Full auto-migration codemod" + "Co-locate as .desktop.tsx in core tree" → both locked in (DEC-12/DEC-13/DEC-14); risks I raised (codemod on subtle files, Tauri code in the web tree, Drawer's dropped swipe/stacking) recorded as accepted with mitigations (reviewed codemod output, web tsconfig/biome excludes, Drawer drift fixed not propagated). [generalizable: no]
- **FB-3** [status: resolved] — "give me an example of how to override a button or a slot or a route" → provided the three worked examples (button→`<Seam>`, slot→existing append + seam-for-render-content, route→`.desktop.tsx` page swap / module swap) + a which-tool-when table; confirmed the design's element-vs-file boundary. [generalizable: no]
- **FB-4** [status: resolved] — "go to phase 5" → entered implementation. [generalizable: no]
- **FB-5** [status: resolved] — "keep going" (continue Phase 5 autonomously) → implemented resolver, relocations, Drawer drift fix, codemod, manifest, docs, 30 unit tests; validated with a green desktop `vite build`. [generalizable: no]

## Notes surfaced during implementation (for the human to see at review)

- **PRE-EXISTING BUG FIXED IN PASSING** (not part of this feature): the desktop
  `vite build` was broken on `origin/main` — `vite-plugin-testid-unique` counted
  `querySelector('[data-testid="x"]')` CSS-selector strings as testid
  declarations, false-flagging `kb-tool-result-{card,toggle}` as duplicates. This
  blocks the desktop build (and merge-gate C1) for ANY branch. Fixed minimally
  (negative lookbehind for `[`). Flagging so you're aware it rode in on this
  branch; it can be cherry-picked out if you'd rather land it separately.
  [generalizable: yes — a testid/attribute scanner must not match CSS attribute
  selectors as declarations; applies to `gen-testid-registry.mjs` too if it ever
  gains a uniqueness check]

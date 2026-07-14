# Chunk `sdk-testinfra` — DRIFT round 1

**Drift count: 0.**

Definition: a divergence between the plan (CUT + TRANSFORMS) and the realized diff
that isn't a declared, resolved decision.

Checked:

- **Every moved generator in the CUT MOVE table exists in the package and is
  `git rm`ed from ziee** — verified: `gen-state-matrix`, `gen-overlay-registry`
  (+test), `gen-gallery-coverage`, `gen-gallery-seed-registry` (+test),
  `gallery-coverage`, `check-gallery-prod-exclusion`, `capture-gallery-{screenshots,states}`
  all under `packages/gallery/scripts/`; the 10 files + 2 vite plugins deleted
  from `src-app/ui`.
- **No divergent duplicate** — the moved generic generators exist ONCE (package);
  ziee references them via repointed npm scripts. `lib/gallery-surfaces.mjs` stays
  in ziee (still used by the NOT-moved `affordance-audit` + `gen-crop-review` — a
  pre-existing package/ziee dup from the prior gallery chunk, out of scope).
- **`gen-testid-registry` deferral is DECLARED** (T-9/D-4 + BOUNDARY), not silent
  drift — it remains in `src-app/ui/scripts` unchanged.
- **B2 templates are NEW** (no source-deletion), consistent with the CUT ("ziee's
  baseline-coupled specs stay app-side").
- **test-e2e is ADDITIVE** — no ziee e2e file changed (verified via `git status`:
  the only ziee edits are package.json ×2 + vite.config.ts + the 12 deletions).
- **No config field left dead** — `testidOut` was added-then-removed with the
  testid deferral (D-3); the surviving new fields (`srcDir`/`overlayKitImports`/
  `tsconfig`) are all consumed by moved generators.

No unresolved drift → proceed.

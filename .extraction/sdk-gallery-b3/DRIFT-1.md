# Chunk `sdk-gallery-b3` — DRIFT round 1

**Drift count: 0.**

Definition: a divergence between the plan (CUT + TRANSFORMS) and the realized diff
that isn't a declared, resolved decision.

Checked:

- **Every CUT-declared deletion is realized** — the 8 desktop framework copies
  (`GalleryPage`/`pages`/`overlays`/`surfaces`/`matrix`/`useGalleryTheme`/`story`/
  `seed`) + the old `ui/scripts/gen-testid-registry.mjs` + the web
  `support/registry-core.ts` shim are all `git rm`'d. `git status` matches the CUT
  MOVE/DELETE tables exactly.
- **Every replacement resolves to `@ziee/gallery`** — `main.tsx` +
  `galleryConfig.ts` + `module-seed.ts` + `mockApi.ts` import the framework from
  the package; tsc = 0 resolves every symbol.
- **No divergent duplicate** — the framework engine exists ONCE (the package);
  desktop holds only thin bindings (galleryConfig / mockApi shim / module-seed).
  The retired shim's pure functions are imported straight from `@ziee/gallery`.
- **The testid deferral is CLOSED, not silently re-deferred** — the generator IS
  moved (T-7/T-8), the kit-migration reconciled (gb3-02, byte-proven), config
  fields added (T-9). No STOP was triggered because there was no genuine conflict.
- **`MODULE_CASSETTE` retained is NOT dead** — it is the internal input to
  `discoverGalleries` (and the parity export mirroring the web registry), declared
  in T-3/D-2. `GALLERY_CASSETTE` (which WAS dead post-rewire) is removed (T-5).
- **No config field left dead** — `kitTestIds` + `testidOut` are both consumed by
  the moved generator; both default to the historical in-app behavior.
- **No Rust/OpenAPI/generated impact** — verified via `git status` (gb3-11): zero
  `.rs`/`openapi.json`/`types.ts`/migration/`.sql` changes; kit `testIds.generated.ts`
  byte-identical; `vendor/pgvector` untouched.
- **Page-focus not silently broadened** — desktop `discoverGalleries` still
  excludes the web overlay/deep/seeded/story entries (gb3-06), matching the prior
  `module-seed.ts` cassette-only inheritance.

No unresolved drift → proceed.

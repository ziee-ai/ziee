# Chunk `sdk-gallery` — TESTS-MOVED

The gallery framework's `node --test` unit files travel with the pure code they
exercise; the rich surface coverage (per-module cassettes, coverage maps, e2e
visual specs) STAYS app-side (it is ziee content). No content test is edited.

- **T-registry-core** [moved→SDK] `support/registry.test.ts` →
  `sdk/packages/gallery/src/registry/registry-core.test.ts`. Covers
  `mergeModuleCassettes` (merge + collision-throw + no-cassette), `assertUniqueSlugs`
  (distinct pass + duplicate throw), `moduleNameFromPath`. Import path
  `./registry-core.ts` already matches the new dir — verbatim move.
- **T-hold** [moved→SDK] `support/index.test.ts` →
  `sdk/packages/gallery/src/runtime/hold.test.ts`. Covers the store-seed
  durability helpers (`holdForever`/`holdPatch`/`whenTrue`). Import `./hold.ts`
  matches — verbatim move.
- **T-mockApi-binary** [moved→SDK] `mockApi-binary.test.ts` →
  `sdk/packages/gallery/src/mock/mockApi-binary.test.ts`. Covers
  `makeBinaryResponse` (bytes + content-type/length) + `base64ToBytes` round-trip.
  Import `./mockApi-binary.ts` matches — verbatim move.

Evidence: `node --test registry/registry-core.test.ts runtime/hold.test.ts
mock/mockApi-binary.test.ts` = **11 pass / 0 fail**.

- **T-content-galleries** [stays→ziee] `modules/*/gallery.tsx` (×36) + `fixtures/**`
  + `coverage.ts`/`stateCoverage.ts` + `stories/**` + the e2e visual specs
  (`tests/e2e/visual/**`) — ziee content, unchanged. The cassette-shape check
  they rely on is preserved by the app's binding alias (proven: wrong-shape
  scratch fails `tsc`).
- **T-equivalence** [stays→ziee] the standalone gallery + the config-driven
  runtime-health run ARE the behavioural equivalence gate (BOUNDARY "Equivalence
  run"): the whole rewired gallery renders through `mountGallery` with identical
  surface counts + 0 new console errors.

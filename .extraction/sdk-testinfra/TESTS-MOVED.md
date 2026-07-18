# Chunk `sdk-testinfra` — TESTS MOVED

The two pure-function unit test files move WITH their generators (they import the
moved `.mjs` by relative path) and are re-run from the package.

| ziee source (DELETED) | SDK dest | imports | result |
|---|---|---|---|
| `ui/scripts/gen-overlay-registry.test.mjs` | `packages/gallery/scripts/gen-overlay-registry.test.mjs` | `./gen-overlay-registry.mjs` (`extractWiredSurfaces`) | 3 tests pass |
| `ui/scripts/gen-gallery-seed-registry.test.mjs` | `packages/gallery/scripts/gen-gallery-seed-registry.test.mjs` | `./gen-gallery-seed-registry.mjs` (`computeSeedDrift`/`hasUserSurface`/`parseSeedExceptions`) | 14 tests pass |

`node --test packages/gallery/scripts/gen-overlay-registry.test.mjs
packages/gallery/scripts/gen-gallery-seed-registry.test.mjs` → **17 pass / 0 fail.**

The pure exported functions are byte-unchanged (only the `isMain` main-block path
resolution was config-driven), so the tests exercise identical logic.

## ziee npm-script repoint (equivalence surface)

`ui/package.json` `test:gallery-seed-registry` repointed to the two package test
files. Verified: `npm run test:gallery-seed-registry` (from `ui/`) → 17 pass.

## Not moved

- `gen-testid-registry.mjs` (+ no dedicated test) — deferred (kit-migration; T-9).
- The heavy Playwright capture/coverage passes (`capture-*`, `gallery-coverage`)
  have no unit tests (they drive a live browser); moved verbatim / config-driven,
  syntax-verified via `node --check`.
- `@ziee/test-e2e` ships as a scaffold LIBRARY; its exit condition is
  `tsc/validate = 0` (the presets/global-setup are exercised by a consuming app's
  suite, not by package-internal tests). No cosmetic self-test was fabricated.

# Chunk F2 — TESTS-MOVED

F2 is a behavior-preserving MOVE of the frontend runtime; there is no new
behavior to prove. Coverage-preservation:

## Moved INTO the SDK (`sdk/packages/framework/src/`)

| Test | Origin | Note |
|---|---|---|
| `stores.test.ts` | ziee `core/stores.test.ts` | `createStoreProxy` specs (action-hook-free, `$` snapshot, render-only reactive reads). Moved byte-for-byte (relative imports `./stores.ts` already). Type-checks green under the framework `tsc`. |
| `store-kit.test.ts` | ziee `core/store-kit.test.ts` | `defineStore`/`defineLocalStore` authoring specs. Byte-for-byte. Type-checks green. |
| `overrides/registry.test.ts` | ziee `core/overrides/registry.test.ts` | Override registry runtime-Map specs. Byte-for-byte. |
| `overrides/seam.test.ts` | ziee `core/overrides/seam.test.ts` | `Seam`/`useOverride` resolution specs. Byte-for-byte. |
| `__test-stubs__/{events,module-system}.ts` | ziee `core/__test-stubs__/*` | node-test boundary stubs, moved with the specs. |

All four specs are `node:test` suites (not tsc-run); they are **type-checked
green** by the framework's `tsc --noEmit` (E4: no behavioral assertion edited).
The specs' node-test RUNTIME loader (which aliases `@/core/*` → the stubs in
ziee) is not yet wired for the framework package — deferred, since **F2's gate is
`tsc --noEmit`**, and the moved sources are relative-imported so no `@/` alias is
needed at type-check time. Noted in BOUNDARY as a follow-up.

## No covering test dropped or renumbered (A5 shrink-guard)

The prior committed manifest (F1) covered the kit; F2 is the first chunk to touch
these four core-runtime specs — none are dropped or renumbered. The specs that
covered the moved code all moved with it; none stayed orphaned in ziee.

## The equivalence gate for this chunk (in place of new tests)

This is a wire-irrelevant frontend MOVE — the E8 openapi/types golden does NOT
apply. The enforced equivalence checks:

| Check | Result |
|---|---|
| `@ziee/framework` standalone `tsc --noEmit` | **exit 0** |
| ziee `ui/` `tsc --noEmit` | **exit 0** |
| ziee `desktop/ui/` `tsc --noEmit` | **exit 0** |
| ziee `ui/` biome `lint:guardrails` (noRestrictedImports) | **exit 0** |
| No `@/` import remains in `sdk/packages/framework/src` | grep: none |
| No ziee ref points at a moved (deleted) file | grep: none |
| Generated `api-client/types.ts` (ui + desktop) vs baseline | byte-identical |

## Stayed in ziee (unchanged behavior)

`core/permissions/*` (7), `core/components/*` (2), `core/sync/types.ts` (1) — kept
app-side (T-5); only their consumed `@/core/*` specifiers were rewritten to
`@ziee/framework/*`. No test for them changed.

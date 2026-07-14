# Chunk F2 — FIX_ROUND-1

Re-audit of the full F2 diff (framework new files + ziee edits) against the
LEDGER angles + the `git diff` hunks.

## Findings raised during implementation (all resolved before the boundary)

These surfaced while converging the standalone framework `tsc` and are already
reflected in TRANSFORMS — they are the equivalence-preserving resolutions, not
outstanding defects:

- **Empty `Slots` → `keyof Slots = never`** broke `module-system/store.ts`. Fixed
  with the `SlotKey` conditional + `Object.entries(slots as Record<string,any[]>)`
  (T-3). ziee typing unchanged (SlotKey resolves to the app union when augmented).
- **Empty `AppEvents`** rejected the `sync:reconnect` emit. Fixed by casting
  `as never` (matches the sibling per-entity emit) (T-3).
- **`callAsync(params: any)`** un-narrowed the GET query encode. Fixed with
  `encodeURIComponent(String(value))` — runtime-identical (T-7).
- **Relative-path `declare module '../../core/stores'`** (8 files) were missed by
  the `@/core/*`-targeted rewrite and cascaded to "Property 'Auth' does not exist
  on RegisteredStores" across ~150 sites. Fixed by rewriting them to
  `@ziee/framework/stores` (T-3 / DRIFT-1.2).
- **`ui/src/index.ts` (@ziee/ui-core barrel)** re-exported the deleted `./core`
  barrel. Repointed at `@ziee/framework` (T-2 / F2-14).

## Confirmed defects in the extraction itself

- Framework standalone `tsc --noEmit`: **exit 0**.
- ziee `ui/` `tsc --noEmit`: **exit 0**.
- ziee `desktop/ui/` `tsc --noEmit`: **exit 0**.
- biome `lint:guardrails` (noRestrictedImports) over ui: **exit 0**.
- Generated `types.ts` (ui + desktop): byte-identical to baseline, unmodified.
- No `@/` reference remains in the framework; no ziee ref points at a moved file.

Re-audit of the post-fix diff surfaced no behavior change beyond the declared
transforms.

**New confirmed findings:** 0

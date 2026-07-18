# Chunk F2 — DRIFT-1

Drift reconciliation after the move. Each check: every moved file ∈ CUT.md;
every changed symbol ∈ TRANSFORMS.md; no ziee ref still points at the old
location; the equivalence tripwires (generated `types.ts`) not red.

## DRIFT-1.1 — no `@/` reference remains in the framework package

`grep -rn "from '@/" sdk/packages/framework/src` + `import('@/` → **0 hits**.
Every moved file's imports are framework-relative (T-1) or a declared framework
generic (T-2). RESOLVED.

## DRIFT-1.2 — no ziee ref points at a moved (deleted) location

`grep` for `@/api-client/core`, `@/api-client/sse-types`, `from '@/core/module`,
`from '@/core/stores`, `from '@/core/events`, `from '@/core/overrides`,
`from '@/core/store-kit` across `ui/src` + `desktop/ui/src` → **0 hits**. The
relative-path augmentations (`declare module '../../core/stores'`, 8 files) were
found mid-drift and rewritten to `@ziee/framework/stores`; re-grep for
`declare module '[.].*core/` → 0 hits. RESOLVED.

## DRIFT-1.3 — every moved file ∈ CUT.md, every deleted file accounted for

`git status` shows 29 deletions under `ui/src/core` + `ui/src/api-client` — all
listed in CUT.md "MOVED"/"del". `sdk/packages/framework/src` has 28 moved-source
files + 3 new infra (index.ts, api-client/index.ts, env.d.ts) — all in CUT.md.
The 10 stay-behind core files (permissions 7, components 2, sync/types 1) are
listed in CUT "Stays app-side" (T-5). RESOLVED.

## DRIFT-1.4 — every changed symbol ∈ TRANSFORMS.md

The only non-import symbol changes are: `callAsync` signature + `createApiClient`
factory (T-2), `SyncEvent` local type (T-2), `setBaseUrlResolver`/`getBaseUrl` DI
(T-4), `SlotKey` + `Object.entries` cast + `as never` emit (T-3),
`encodeURIComponent(String(value))` (T-7), and the thin app `index.ts` binding
(T-2). All declared. RESOLVED.

## DRIFT-1.5 — equivalence tripwire (generated types) not red

`api-client/types.ts` (ui + desktop) is byte-identical to
`.extraction/baseline/types.{ui,desktop}.ts` and unmodified in the changeset (F2
touches no route/type/schema). The E8 openapi/types golden does not apply to this
wire-irrelevant chunk; the tsc-clean gate is the equivalence proof. RESOLVED.

**Unresolved drifts:** 0

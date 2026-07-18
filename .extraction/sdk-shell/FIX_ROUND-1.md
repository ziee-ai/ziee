# Chunk `sdk-shell` — FIX round 1

The blind audit (LEDGER, 12 angles incl. equivalence + security) surfaced no
behavioural regression. Three mechanical issues were caught + fixed DURING
implementation (pre-commit), logged for the record:

- **FIX-1.1** — the new `@ziee/shell` package tsc followed the `@ziee/kit` CSS
  side-effect import (`overlayscrollbars/overlayscrollbars.css`) + `import.meta.env`
  and errored under its own tsconfig, and could not resolve `@ziee/framework/*`
  subpaths. Added `src/env.d.ts` (`declare module '*.css'` + `ImportMeta.env`) and
  `paths` for `@ziee/{kit,framework}` in `sdk/packages/shell/tsconfig.json`,
  mirroring the gallery package exactly. shell tsc = 0 after.

- **FIX-1.2** — ziee `ui/` + `desktop/ui/` could not resolve `@ziee/shell` (both
  tsconfigs resolve `@ziee/*` via `paths`, not package exports). Added
  `@ziee/shell` + `@ziee/shell/*` path mappings to BOTH `ui/tsconfig.json` and
  `desktop/ui/tsconfig.json`. ui tsc = 0 + desktop tsc = 0 after.

- **FIX-1.3** — baseline `@ziee/framework` tsc was RED on a PRE-EXISTING, unrelated
  test-only type bug (`router/config.test.ts`: an inline `RoutePermissionGate` had
  `children: unknown` not assignable to `ReactNode`). Fixed the one-line annotation
  to `children: import('react').ReactNode` so the framework-tsc=0 gate for the new
  `permissions` subpath is actually meetable. Disclosed as a baseline cleanup, not
  a feature change.

Two DELIBERATE, non-regression transforms (declared T-1/T-2, not fixes): the
permission leaf widened enum→`string` (app-shim re-narrows) and the auth/config/
layout store reads became typed local casts on the framework's own `Stores`
(runtime read byte-identical). The verification run's overwrite of ziee's committed
`RUNTIME_FINDINGS.md` was reverted via `git checkout` (verified clean after);
`RUNTIME_FINDINGS.jsonl` is gitignored (generated).

**New confirmed findings:** 0

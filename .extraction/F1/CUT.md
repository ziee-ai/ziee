# Chunk F1 — `@ziee/kit` (shadcn kit + design tokens + testIds) — CUT manifest

Move ziee's design-system kit — `src-app/ui/src/components/ui/*` (the shadcn
vendored primitives + our kit components + the barrel + `testIds.generated.ts` +
the kit docs) — into `sdk/packages/kit/src/` as the `@ziee/kit` npm workspace
package, **equivalence-preserving**. Component source is moved BYTE-PRESERVED
(only import specifiers rewritten); ziee's `ui/` + `desktop/ui/` then consume the
kit from `@ziee/kit`. This chunk also proves the npm-submodule wiring
(`sdk/packages/*` workspaces resolve as symlinks).

## Design gate — the kit is domain-agnostic + self-contained

The kit must not import back into ziee's `@/` app domain. Three outward `@/`
edges existed (`@/lib/utils`, `@/hooks/use-mobile`, `@/components/common/DivScroll{X,Y}`)
plus kit-internal `@/components/ui/*` self-references. All are resolved so the
package names **zero** app types/stores. The one genuine DOMAIN coupling
(`DivScrollY` → `Stores.AppLayout`) is reported + severed (TRANSFORMS T-4).

**Gate (this chunk is wire-irrelevant → tsc-clean, not the openapi golden):**
`@ziee/kit` builds standalone (`tsc --noEmit` = 0); ziee `ui/` + `desktop/ui/`
`tsc --noEmit` = 0 (imports resolve, types clean). No route/type/schema is
touched, so the E8 openapi/types golden does not apply here.

## Files — MOVED INTO `sdk/packages/kit/src/` (submodule `sdk/`)

- `src/kit/*.{tsx,ts}` (63 files) — our kit components + hooks/utilities
  (`use-controllable-state`, `value-binding`, `table-view-core[.test]`,
  `safe-href`, `style-guard`, …), moved byte-for-byte; only `@/…` import
  specifiers rewritten to kit-relative (`../lib/utils`, `../shadcn/*`,
  `../hooks/use-mobile`, `../internal/div-scroll-*`).
- `src/shadcn/*.tsx` (54 files) — the CLI-vendored shadcn primitives, byte-for-byte;
  only `@/lib/utils` → `../lib/utils`, `@/components/ui/shadcn/*` → `./*`,
  `@/hooks/use-mobile` → `../hooks/use-mobile` rewritten.
- `src/index.ts` — the public barrel, byte-for-byte except the line-1 doc comment
  (`Import from '@/components/ui'` → `'@ziee/kit'`).
- `src/testIds.generated.ts` — the typed testid registry, byte-for-byte.
- `src/KIT_MANIFEST.md`, `src/ARCHITECTURE.md`, `src/DESIGN_DIRECTION.md`,
  `src/TOKEN_MAP.md` — kit docs, byte-for-byte.

## Files — NEW in the kit (self-contained dependencies)

- `src/lib/utils.ts` — the `cn` class-merge helper, **byte-identical** copy of
  `ui/src/lib/utils.ts` (clsx + tailwind-merge; the util the whole kit uses).
- `src/hooks/use-mobile.ts` — `useIsMobile`, **byte-identical** copy of
  `ui/src/hooks/use-mobile.ts` (pure viewport hook; used by `shadcn/sidebar`).
- `src/internal/div-scroll-x.tsx` — **byte-identical** copy of
  `ui/src/components/common/DivScrollX.tsx` (pure OverlayScrollbars wrapper; used
  by `kit/tabs`).
- `src/internal/div-scroll-y.tsx` — a store-FREE subset of the app's
  `DivScrollY` (drops the `Stores.AppLayout` read; the kit's only consumer,
  `kit/dialog`, never used the store-driven `nativeFlow` path → render-equivalent).
  See TRANSFORMS T-4.
- `src/styles/tokens.css` — the shadcn design-token layer (`@theme inline` +
  `:root` + `.dark`), extracted **byte-identical** from `ui/src/index.css` lines
  47–205, so the package ships its own token contract.
- `src/css.d.ts` — ambient `declare module '*.css'` for the side-effect CSS
  import in `kit/scroll-area` (ziee gets this from `vite/client`; the bundler-
  agnostic kit declares it itself).
- `package.json` — `@ziee/kit`: `exports` (barrel `.` + `./*` subpath +
  `./styles/tokens.css`), third-party deps (radix/base-ui, cva, clsx,
  tailwind-merge, lucide, sonner, vaul, overlayscrollbars, …), react/react-dom
  peers, `@types/*` devDeps.
- `tsconfig.json` — bundler resolution, `jsx: react-jsx`, DOM libs,
  `allowImportingTsExtensions` (for `table-view-core.test.ts`).

## Files — CHANGED IN ziee (submodule `src-app/`, NOT committed here)

- **del:** `src-app/ui/src/components/ui/` (121 files) — the entire moved slice.
- **edit:** 348 files under `src-app/ui/src/` + 15 under `src-app/desktop/ui/src/`
  + `src-app/ui/tests/e2e/testid.ts` — import specifier `@/components/ui…` →
  `@ziee/kit…` (barrel and deep), quote-anchored (only import specifiers change).
- **edit:** `src-app/ui/package.json`, `src-app/desktop/ui/package.json` — add
  `"@ziee/kit": "*"` workspace dep.
- **edit:** `src-app/ui/tsconfig.json`, `src-app/desktop/ui/tsconfig.json` — add
  `@ziee/kit` + `@ziee/kit/*` `paths` (tsc doesn't append extensions to `exports`
  wildcard targets; the repo's established `@ziee/ui-core/*` pattern is reused).

## Stays app-side (each app owns)

`ui/src/lib/utils.ts` (the app's `cn`, used by ~hundreds of app files),
`ui/src/hooks/use-mobile.ts`, `ui/src/components/common/DivScroll{X,Y}.tsx` (used
app-wide, and `DivScrollY` reads the `AppLayout` store) — all UNCHANGED. The kit
vendors its own self-contained copies rather than reaching into them, so app-side
behavior is untouched. `ui/src/index.css` (the app's Tailwind entry + the design-
system single-source-of-truth for `gen:design-spec`) — UNCHANGED; the kit's
`tokens.css` is an additive copy.

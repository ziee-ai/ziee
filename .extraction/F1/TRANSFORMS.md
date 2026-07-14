# Chunk F1 — TRANSFORMS

Every non-byte-identical change applied while moving the kit into `@ziee/kit`,
each with the design decision + resolution. Zero TBD.

## T-1 — component source moved BYTE-PRESERVED; only import specifiers rewritten

All 117 component files (63 `kit/*`, 54 `shadcn/*`) + the barrel + the testid
registry moved with their bodies byte-for-byte. The only in-file edits are import
**specifiers** that named the old `@/` locations, rewritten to kit-relative:
`@/lib/utils` → `../lib/utils`; `@/components/ui/shadcn/X` → `../shadcn/X` (from
`kit/`) / `./X` (from `shadcn/`); `@/hooks/use-mobile` → `../hooks/use-mobile`;
`@/components/common/DivScrollY|X` → `../internal/div-scroll-y|x`.

**Resolution:** a per-file diff of every moved file against its `HEAD` original
reports **0 non-import differing lines** across 117 files (22 byte-identical, 95
import-only). The barrel `index.ts` differs only in its line-1 doc comment
(`'@/components/ui'` → `'@ziee/kit'`). Verified: DRIFT-1.1.

## T-2 — the `cn` util, `use-mobile`, and `DivScrollX` vendored into the kit

### Decision — how the kit gets its self-contained micro-deps without a `@/` back-import

The kit uses `@/lib/utils` (`cn`, 94 sites), `@/hooks/use-mobile` (sidebar), and
`@/components/common/DivScrollX` (tabs). All three are **pure, domain-free** UI
infra, but they live OUTSIDE the moved `components/ui/*` slice and are used
app-wide (`cn` in hundreds of files; `DivScrollX`/`use-mobile` in 3 each). Moving
them wholesale would balloon the chunk and churn unrelated app sites; leaving the
kit importing `@/…` would break package self-containment (a domain-agnostic SDK
package cannot reach into the consumer app's `@/` alias).

**Resolution:** the kit **vendors its own byte-identical copies**
(`src/lib/utils.ts`, `src/hooks/use-mobile.ts`, `src/internal/div-scroll-x.tsx`),
verified `diff`-identical to their app originals. The app keeps its own copies
UNCHANGED (they remain the app-wide source). This is the standard way a component
library owns its primitives; the small duplication is intentional and confined to
three tiny pure files (not a divergent duplicate of a MOVED slice file). Reported
in BOUNDARY.

## T-3 — kit-internal `@/components/ui/*` self-references → relative

### Decision — the slice referenced itself through the `@/` alias

Within the slice, kit components imported sibling shadcn primitives via the
absolute alias (`@/components/ui/shadcn/button`, etc.) as well as relative paths.
Inside a package these must be relative.

**Resolution:** rewritten per source directory — from `src/kit/*`,
`@/components/ui/shadcn/X` → `../shadcn/X` and `@/components/ui/kit/X` → `./X`;
from `src/shadcn/*`, `@/components/ui/shadcn/X` → `./X` and `@/components/ui/kit/X`
→ `../kit/X`. A full-tree grep confirms **zero** `from '@/…'` / `import('@/…')`
remain anywhere in `sdk/packages/kit/src` (DRIFT-1.2).

## T-4 — `DivScrollY` → `Stores.AppLayout` domain coupling severed (the reported coupling)

### Decision — the kit's dialog needs a vertical scroller, but `DivScrollY` reaches an app store

`kit/dialog.tsx` imports `DivScrollY`, and `DivScrollY`
(`components/common/DivScrollY.tsx`) reads `Stores.AppLayout?.nativeScroll` (from
`@/core/stores`) to opt tall mobile scrollers into the document scroll when its
`nativeFlow` prop is set. That store read is the ONE genuine coupling of the kit
to an **app module** (`AppLayout`). Moving `DivScrollY` wholesale into the kit is
not viable (it is used in 19 app files and its `nativeFlow` behavior depends on
the app store); dragging the store read into the kit would leave the package
domain-coupled, defeating the gate.

**Resolution:** the app-side `DivScrollY` STAYS app-side, UNCHANGED (all 19 app
usages keep their exact behavior). The kit ships a store-free subset
`src/internal/div-scroll-y.tsx`: identical rendering to `DivScrollY`'s
non-`nativeFlow` path, with the `Stores.AppLayout` import + the `nativeFlow &&
nativeScroll` branch removed (the `nativeFlow` prop is retained for shape-parity
but ignored). The kit's only consumer, `kit/dialog.tsx`, **never passes
`nativeFlow`** (verified: it calls `<DivScrollY className=… options=… >`), so the
removed branch produced no output there → the dialog renders byte-equivalently.
The kit now names no app store. This is the coupling the gate asked to find +
report.

## T-5 — design tokens shipped as a kit asset (`tokens.css`)

### Decision — "include the design tokens the kit needs" without forking the single source of truth

The kit's components render against the shadcn CSS variables
(`bg-primary`/`text-muted-foreground`/`rounded-md`/…). Those variables are Tailwind
class *strings* resolved by the app's build, not TS imports — so no `@/` token
import exists to break, and the tsc gate is token-agnostic. But the package should
still carry its own token contract to be self-describing for a future consumer.
CLAUDE.md designates `ui/src/index.css` the single source of truth (input to
`gen:design-spec`), so a second *authoritative* source would be drift.

**Resolution:** `src/styles/tokens.css` is an **additive, byte-identical** extract
of `index.css` lines 47–205 (`@theme inline` + `:root` + `.dark`), exposed via the
package's `./styles/tokens.css` export. `ui/src/index.css` is UNCHANGED and stays
the SoT; ziee keeps importing it, so nothing about ziee's build or the design-spec
check changes. The kit copy documents its provenance in a header comment.

## T-6 — package `exports` + consumer `paths` (module resolution)

### Decision — extensionless deep subpaths don't resolve through `exports` under tsc

Deep imports (`@ziee/kit/kit/table`, `@ziee/kit/shadcn/field`,
`@ziee/kit/testIds.generated`) are extensionless. The package `exports` wildcard
`"./*": "./src/*"` maps them to an extensionless target, and `tsc`
(`--traceResolution` confirmed) does **not** append `.tsx`/`.ts` to an `exports`
wildcard target (unlike a `.js`-suffixed target it would strip+substitute), so the
subpaths fail to resolve — while the barrel `.` resolves fine. Runtime bundlers
(Vite/esbuild/rollup) DO append extensions here, so `exports` is correct for
runtime.

**Resolution:** keep the package `exports` (`.` barrel + `./*` subpath +
`./styles/tokens.css`) for runtime bundlers, and add a consumer-side `paths`
mapping in BOTH `ui/tsconfig.json` and `desktop/ui/tsconfig.json`
(`@ziee/kit` → `…/sdk/packages/kit/src/index.ts`, `@ziee/kit/*` →
`…/sdk/packages/kit/src/*`) so tsc resolves every subpath uniformly (paths targets
DO get extension substitution). This is the identical split the repo already uses
for `@ziee/ui-core`/`@ziee/ui-core/*` in `desktop/ui/tsconfig.json`. All three
`tsc --noEmit` runs then exit 0 (DRIFT-1.5).

## T-7 — consumer import specifiers repointed; slice deleted from ziee

**Resolution:** a quote-anchored replace (`'@/components/ui` → `'@ziee/kit`,
`"@/components/ui` → `"@ziee/kit`) over `ui/src` (348 files), `desktop/ui/src`
(15 files), and `ui/tests/e2e/testid.ts` (1, from its relative path). Only import
specifiers change; component behavior/appearance is untouched. The moved slice is
`git rm`-ed from ziee (121 files) — single source of truth (E6). `@ziee/kit`
added as a workspace dep in both consumer `package.json`s. Verified: no
`@/components/ui` import specifier remains in either tree (DRIFT-1.4).

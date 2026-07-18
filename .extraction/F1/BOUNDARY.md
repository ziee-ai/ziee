# Chunk F1 — BOUNDARY

What `@ziee/kit` may and may not name, and the evidence the move keeps the kit
domain-agnostic.

## The kit is domain-free

- The package imports **only** third-party libs (react/react-dom peers,
  base-ui/@radix-ui/react-slot, cva, clsx, tailwind-merge, lucide-react, sonner,
  vaul, overlayscrollbars(+react), @tanstack/react-virtual, react-hook-form/
  @hookform/resolvers, react-day-picker, date-fns, input-otp, embla-carousel-react,
  cmdk, next-themes, zod) + its own kit-relative modules. Grep of
  `sdk/packages/kit/src` for `from '@/`, `Stores.`, `@/core`, `@/modules`,
  `@/api-client`, `@ziee/framework`, `@ziee/ui-core` returns **zero** real hits.
- No app `Config`, no store, no domain module, no generated `api-client` type is
  named anywhere in the kit.

## The one coupling that existed — found, reported, severed

`kit/dialog.tsx` → `DivScrollY` → `Stores.AppLayout.nativeScroll` was the single
edge coupling the kit to an **app module** (`AppLayout`, via `@/core/stores`). This
is the coupling the gate asked to surface. Resolution (TRANSFORMS T-4): the
app-side `DivScrollY` stays app-side unchanged; the kit ships a store-free
`internal/div-scroll-y.tsx` whose non-`nativeFlow` render path is identical, and
the dialog never passed `nativeFlow`, so the dialog renders equivalently while the
package carries no app store. **The kit is therefore fully domain-agnostic.**

## Vendored micro-deps (the deliberate, confined duplication)

`cn` (`lib/utils.ts`), `useIsMobile` (`hooks/use-mobile.ts`), and `DivScrollX`
(`internal/div-scroll-x.tsx`) are byte-identical copies of app helpers that live
OUTSIDE the moved slice and are used app-wide. The kit owns its own copies (a
library vendoring its primitives) rather than importing `@/…`; the app keeps its
originals unchanged. This is not a divergent duplicate of a MOVED-slice file.

## What stays app-side (the boundary line)

`ui/src/lib/utils.ts`, `ui/src/hooks/use-mobile.ts`,
`ui/src/components/common/DivScroll{X,Y}.tsx` (all app-wide, and `DivScrollY` is
store-coupled), and `ui/src/index.css` (the app's Tailwind entry + the
`gen:design-spec` single source of truth) all stay in ziee, unchanged. The kit's
`tokens.css` is an additive byte-identical copy — a self-contained token contract,
not a second authority.

## Consumption

ziee `ui/` + `desktop/ui/` consume the kit from `@ziee/kit` (barrel `.` + deep
subpaths). Resolution is dual, matching the repo's existing `@ziee/ui-core`
pattern: consumer `tsconfig` `paths` for tsc, package `exports` for runtime
bundlers. `npm install` wires `node_modules/@ziee/kit` → `sdk/packages/kit` as a
symlink — the F1 npm-submodule-wiring milestone.

## Gate

This chunk touches **no** route/type/schema, so it is wire-irrelevant and the E8
openapi/types golden does not apply. The gate is **tsc-clean**: `@ziee/kit`,
ziee `ui/`, and ziee `desktop/ui/` each `tsc --noEmit` = exit 0.

## Known follow-up (out of the F1 tsc gate)

`ui/scripts/{gen-kit-manifest,gen-testid-registry,classify-gallery-coverage}.mjs`
still scan `src/components/ui`, so the full `npm run check` sub-gates
`check:kit-manifest` / `check:testid-registry` (and the gallery-coverage
classifier) need their scan roots repointed at the kit's new home. This is
deliberately deferred (F1's gate is `tsc --noEmit`, per the plan's "this chunk is
wire-irrelevant"); it does not affect any of the three tsc gates.

# Desktop raw-shadow exceptions (permanent, committed to main)

The raw-shadow gate (`gen-override-registry.mjs --check`, run in `npm run check`
for both UI workspaces) fails on any desktop-tree file that shadows a
`src-app/ui` path unless it is a `<Seam>` registration, a co-located
`.desktop.{tsx,ts}`, a desktop-exclusive module, or an approved
`SHADOW-EXCEPTION` listed HERE.

This file is the PERMANENT source of truth for those approvals. It lives in the
product tree (NOT under `.lifecycle/`, which is stripped at merge) precisely so
the gate keeps working on `main` for everyone. To add an exception, a desktop
file must be structurally un-migratable (not merely inconvenient) — give a
concrete reason and record the sign-off.

Format the gate parses (one per line):
`- SHADOW-EXCEPTION: <path-relative-to-desktop/ui/src> — <reason> [approved: <who/when>]`

- SHADOW-EXCEPTION: main.tsx — the desktop entry point, loaded DIRECTLY by index.html (`<script src="./main.tsx">`), never through the `@/` resolver, so neither a `.desktop.tsx` (tier-2 fires only for `@/` imports) nor a `<Seam>` (it renders the root, not an element) can apply [approved: user 2026-07-11]
- SHADOW-EXCEPTION: modules/memory/module.tsx — a `module.tsx` is discovered by `import.meta.glob`, which bypasses the `@/` resolver; a core-tree `module.desktop.tsx` is found by neither `desktop-loader.ts` (globs the desktop tree) nor core `loader.ts` (globs the literal `module.tsx`). It stays a glob-discovered desktop-tree module [approved: user 2026-07-11]
- SHADOW-EXCEPTION: api-client/types.ts — a GENERATED file: the desktop `--generate-openapi` binary writes the desktop-specific OpenAPI types here (the desktop tsconfig special-cases `@/api-client/types` to it). It is machine-generated, not a hand-written override [approved: user 2026-07-11]

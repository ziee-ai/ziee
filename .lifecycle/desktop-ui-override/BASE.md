# BASE.md — conflict-surface scoping

Branch: `feat/desktop-ui-override` off `origin/main` @ `ca3e7d62f`.

## What current main touches that this branch also touches

- **Migrations:** none. This is a pure-frontend feature. Highest existing
  migration is `00000000000154_add_voice_streaming_settings.sql`; this branch
  adds **no** migration → zero migration-number collision risk.
- **OpenAPI / `api-client/types.ts`:** no backend type change → **no**
  `just openapi-regen` implied for either workspace. The generated
  `api-client/types.ts` (both trees) is untouched.
- **Frontend workspaces touched (expanded — aggressive path approved):**
  - `src-app/ui/**` — NEW `src/core/overrides/` module + `<Seam>` primitive; the
    5 class-B core components gain a declared seam; 5 class-A `.desktop.tsx`
    files land here (relocated from the desktop tree); NEW
    `scripts/seam-codemod.mjs` + `scripts/gen-override-registry.mjs`; EDIT web
    `tsconfig.json` + `biome.json` (`**/*.desktop.*` exclude) and
    `plugins/vite-plugin-local-override.ts` is edited in the **desktop**
    workspace (see below).
  - `src-app/desktop/ui/**` — EDIT `plugins/vite-plugin-local-override.ts`
    (`.desktop.*` probe tier) + `plugins/vite-plugin-testid-unique.js` (`.desktop`
    shadow awareness); a desktop override-registration entry module; desktop seam
    variants for the 5 class-B conversions; DELETE the 5 class-B desktop shadows;
    the 5 class-A desktop shadows are `git mv`d into the core tree as
    `.desktop.tsx`.
- **Shared generators:** `gen-testid-registry.mjs` / `gen-kit-manifest.mjs` /
  `gen-design-spec.mjs` are consumed unchanged. The new
  `gen-override-registry.mjs` follows their pattern and writes a manifest into
  the **core** tree only (like `testIds.generated.ts`), read in `--check` mode
  by both workspaces.

## Files main is actively changing that we also edit

None identified. The core framework `src/core/` and the desktop
`plugins/vite-plugin-local-override.ts` have been stable across recent commits.
The seam-declaring edits to core components are additive and localized.

## Merge-gate pre-check expectations

- C2 migration-collision: N/A (no migration).
- C3 regen-parity: N/A (no OpenAPI change).
- C1 clean build: both `ui` and `desktop/ui` must `tsc`/`vite build` clean; the
  desktop build renders BOTH trees (core-fallback via `localOverridePlugin`),
  so the seam must typecheck under the desktop tsconfig's `@/*`→core mapping.

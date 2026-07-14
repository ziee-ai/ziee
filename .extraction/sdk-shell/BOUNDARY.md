# Chunk `sdk-shell` — BOUNDARY

- **E1** (CUT present, ≥1 move: line, 3-bucket map): PASS — CUT.md has the
  A/B/C bucket map + a per-file MOVE table (source→dest) + the FE-8 severance
  proof + the NO-Rust/OpenAPI gate.
- **E2** (TRANSFORMS: every differing symbol has a T-N; Decision Resolution;
  no TBD): PASS — T-1..T-7 + D-1..D-5 all RESOLVED, zero TBD.
- **E3** (LEDGER valid, ≥8 angles, includes equivalence + security): PASS — 12
  entries across 12 distinct angles incl. `security` (shl-02: the `<Can>`/wildcard
  gating semantics) + `equivalence` (shl-03 auth-seam proof, shl-06 live render).
- **E4** (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS — see
  AUDIT_COVERAGE.tsv (every changed file × ≥3 angles).
- **E5** (move-completeness: every dest exists; every symbol resolves): PASS —
  every BUCKET-A/B dest exists under `sdk/packages/shell/src/` +
  `sdk/packages/framework/src/permissions/`; shell tsc=0 + framework tsc=0 resolve
  every symbol; ui tsc=0 + desktop tsc=0.
- **E6** (source-deletion: moved code absent from ziee as a divergent duplicate):
  PASS — every moved file's old `@/` path is a byte-thin re-export SHIM (the
  equivalence mechanism), NOT a divergent copy. `App.tsx` is the only non-shim
  rewrite (a thin `AppShell` consumer). No moved implementation survives app-side.
- **E7** (transform-declared: every differing moved symbol has a T-N): PASS
  (T-1..T-7).
- **E8** (no Rust/OpenAPI/generated-types impact): PASS — `git status` shows zero
  changes to any `api-client/types.ts` / `openapi.json` / Rust file.
- **DRIFT-1 unresolved: 0. FIX_ROUND-1 new findings: 0.**

## Declared follow-ups (out of THIS chunk's scope)

- **B-1 — app-layout + settings SHELL scaffold move (DEFERRED; the FE-7 tail).**
  The biggest remaining copy-cost surfaces — `modules/layouts/app-layout/**` (the
  sidebar + the 7 slot definitions: `sidebarNavigation/Tools/PrimaryActions/
  Content/Bottom/Footer/appBanners`) and `modules/settings/**` (SettingsPage,
  SettingsLayout, SettingsPageContainer, `settingsUserPages`/`settingsAdminPages`
  slots) — were NOT moved this chunk. **Cut-line / the tangle:** they carry
  `.desktop.tsx` platform variants (`LeftSidebar/Drawer/HeaderBarContainer/
  SidebarToggleButton.desktop.tsx`, `SettingsPage.desktop.tsx`) that the desktop
  `localOverridePlugin` swaps **only for `@/`-prefixed specifiers**. A package's
  internal imports are relative, so `@ziee/shell`'s AppLayout would always resolve
  the WEB variant on desktop — a silent desktop-render regression (equivalence
  violation) not caught by the web-only `gate:ui`. Two remediation options for the
  follow-up chunk, both to be **verified on a desktop host**: (a) invert the 4
  platform-variant components (+ SettingsPage) to a `@ziee/shell` config/slot seam
  the app fills via `@/`-swapped imports (keeps the existing override mechanism at
  the injection site); or (b) extend `localOverridePlugin` to resolve `.desktop`
  infixes for files under the `@ziee/shell` package src (desktop-build-only).
  The 7 slot TYPE decls travel with whichever option lands. `blank` layout
  (desktop-variant-free) DID move this chunk as the proof-of-pattern.

- **B-2 — ziee `modules/router` → `@ziee/framework/router` migration.** Router is
  already extracted (`@ziee/framework/router`, with a `RoutePermissionGate` DI seam
  that the new `@ziee/framework/permissions` `Can`/`usePermission` can build). ziee
  still runs a parallel local `modules/router` copy; wiring ziee onto the framework
  router (and injecting a permission gate built from the primitives) is a separate
  chunk, untouched here.

- **B-3 — FE-9 sync subscribe handler** (unrelated to shell) remains app-side per
  SDK_GAPS; not in scope.

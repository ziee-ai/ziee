# Chunk `sdk-desktop-shell` — BOUNDARY

Lands **B-1** from `sdk-shell` BOUNDARY.md: the deferred app-layout + settings
SHELL scaffold move, with the DESKTOP `.desktop.tsx` override PRESERVED via
**remediation (a)** (slot/config-seam inversion) and PROVEN on Linux via xvfb.

- **E1** (CUT present, ≥1 move: line, 3-bucket map): PASS — CUT.md has the A/B/C
  bucket map + a per-file MOVE table (source→dest) + the remediation-(a) decision
  + the NO-Rust/OpenAPI gate.
- **E2** (TRANSFORMS: every differing symbol has a T-N; Decision Resolution; no
  TBD): PASS — T-1..T-6 + D-1..D-4 all RESOLVED, zero TBD. Includes the §xvfb
  desktop-override evidence table.
- **E3** (LEDGER valid, ≥8 angles, includes equivalence + security): PASS — 10
  entries across distinct angles incl. `security` (dsh-06: slot permission field +
  appBanners gating) + `equivalence` (dsh-03 store-seam proof) + the load-bearing
  `platform-override` (dsh-01/02: the xvfb desktop-swap proof).
- **E4** (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS — see
  AUDIT_COVERAGE.tsv (every changed file × ≥3 angles).
- **E5** (move-completeness: every dest exists; every symbol resolves): PASS —
  every BUCKET-A dest exists under `sdk/packages/shell/src/`; shell tsc=0 + ui
  tsc=0 + desktop tsc=0 resolve every symbol + shim.
- **E6** (source-deletion: moved code absent from ziee as a divergent duplicate):
  PASS — the moved app-layout/settings files became a thin injection wrapper
  (AppLayout.tsx) or byte-thin re-export shims (types.ts, useWindowMinSize.ts,
  SettingsPageContainer.tsx). No moved implementation survives app-side as a
  divergent copy. `useMainContentMinSize` is app-only (never moved).
- **E7** (transform-declared: every differing moved symbol has a T-N): PASS
  (T-1..T-6).
- **E8** (no Rust/OpenAPI/generated-types impact): PASS — `git status` shows zero
  changes to any `api-client/types.ts` / `openapi.json` / Rust file.
- **DESKTOP OVERRIDE (the whole point): PASS** — verified on Linux via xvfb:
  `resolveOverridePath` → `.desktop` for all injection specifiers; the actual
  `xvfb-run vite build` of desktop/ui bundles the desktop-variant-unique markers
  and excludes the web ones (TRANSFORMS §xvfb). The `.desktop.tsx` variants are
  NOT silently replaced by the web ones.
- **DRIFT-1 unresolved: 0. FIX_ROUND-1 new findings: 0.**

## Declared follow-ups (out of THIS chunk's scope)

- **B-1-residual — SettingsPage body + `Drawer`/`HeaderBarContainer` remain
  app-side (Decisions D-3/D-4).** These are NOT a shell copy-cost a 2nd app pays
  for the generic scaffold: `SettingsPage` is ziee-specific (repo URLs, onboarding,
  RBAC filter, whole-file `.desktop` single-admin divergence); `Drawer`/
  `HeaderBarContainer` are app-wide platform-variant primitives whose desktop `@/`
  swap must stay app-side for their 40+/14 consumers. A future chunk could move
  `Drawer`/`HeaderBarContainer` to `@ziee/shell` ONLY if paired with remediation
  (b) (teaching `localOverridePlugin` to resolve `.desktop` for package src) so
  their consumers can import from the package and still get the desktop variant —
  the exact tradeoff B-1 flagged, deferred here in favour of the clean (a) path.

- **B-2 — ziee `modules/router` → `@ziee/framework/router` migration.** Unchanged
  (still a parallel local copy). Separate chunk.

- **B-3 — FE-9 sync subscribe handler** remains app-side per SDK_GAPS. Not in scope.

# PLAN — desktop-ui-guardrail-parity

## Context (verified)

`src-app/desktop/ui` shares the server UI's source through a FALLBACK `@/` alias
(desktop `src` first, else `../../ui/src`) and ships **0 kit components of its
own**, so every SHARED surface already inherits the server-ui fixes + audit. Two
gaps remain:

- **(a) Guardrail gap** — desktop's `npm run check` runs only ~40% of server-ui's
  gates. Missing from desktop `check`: `lint:adjacent-inline`, `lint:icon-action`,
  `check:kit-manifest`, `check:testid-registry`, `check:design-spec`,
  `check:overlay-registry`. Missing audit tooling entirely (no script files):
  `gallery-geometry-audit.mjs`, `detector-acceptance.mjs`, `affordance-audit.mjs`
  (+allowlist), `gen-crop-review-manifests.mjs`, `gen-overlay-registry.mjs`.
- **(b) Audit gap** — desktop's OWN modules (updater, tunnel-auth, remote-access,
  window/title-bar chrome, host-mount, file-dialog, desktop-base, auth, layouts)
  were never run through the geometry/detector/affordance/vision audit.

**Reuse-vs-copy law (verified by reading each script's path resolution):**
- Scripts that scan BOTH roots or emit a SHARED artifact are workspace-agnostic
  → desktop `check` references `../../ui/scripts/<x>.mjs` (the pattern desktop
  already uses for `lint:colors`, `lint:settings-field`, …). Verified dual-root:
  `lint-adjacent-inline.mjs` (ROOTS = ui/src + desktop/ui/src), `lint-icon-action.mjs`
  (same), `gen-testid-registry.mjs` (scans UI_SRC + DESKTOP_SRC → server-ui file).
  Verified shared artifact: `gen-kit-manifest.mjs` (server-ui kit — desktop has
  none), `gen-design-spec.mjs` (repo-root DESIGN_SYSTEM.md).
- Scripts that resolve their target via `__dirname/../src/dev/gallery` are
  gallery-relative → a COPY placed in `desktop/ui/scripts/` auto-points at the
  DESKTOP gallery (the pattern desktop already uses for its own copies of
  `gen-state-matrix.mjs`, `gen-gallery-coverage.mjs`, `gen-crawl-cassette.mjs`,
  `runtime-health.mjs`, `gate-ui.mjs`). Verified gallery-relative:
  `gallery-geometry-audit.mjs`, `affordance-audit.mjs`, `detector-acceptance.mjs`,
  `gen-crop-review-manifests.mjs`, `gen-overlay-registry.mjs`.

**Gallery visibility (verified):** desktop gallery enumerates pages from the
router store, so desktop-only ROUTE surfaces already render: `settings-about`
(updater), `settings-remote-access`, `settings-host-mount`, `auth-magic`(+detail)
(tunnel-auth). Desktop-only NON-route chrome/overlays are NOT yet in the gallery:
`window` title-bar (routes commented out — chrome), `file-dialog` (routes:[],
imperative overlay), `desktop-base` (bootstrap, no visual), `auth` AuthGuard,
`layouts` app-layout. These need gallery story/overlay entries to be auditable.

## Items

- **ITEM-1**: Backfill the reuse-based static gates into desktop `npm run check` by
  referencing `../../ui/scripts/`: add `lint:adjacent-inline`, `lint:icon-action`,
  `check:kit-manifest`, `check:testid-registry`, `check:design-spec` script
  entries + chain them into `check`. No new script files (dual-root/shared).
- **ITEM-2**: Copy `gallery-geometry-audit.mjs` into `desktop/ui/scripts/`
  (auto-points at desktop gallery; default `dev-gallery.html` entry is served by
  desktop vite — verified); add npm scripts `gallery:geometry` +
  `gallery:geometry:gate`; create empty `desktop/ui/src/dev/gallery/geometry-allowlist.json`.
  (Standalone npm scripts — parity with web, whose `gate-ui.mjs` also does NOT run
  geometry inline; see DRIFT-1.)
- **ITEM-3**: Copy `affordance-audit.mjs` + `affordance-audit-allowlist.json` into
  `desktop/ui/scripts/`; add npm script `gallery:affordance` (standalone, parity
  with web).
- **ITEM-4**: Copy `detector-acceptance.mjs` + the `__detector_fixtures__/` gallery
  fixtures into desktop (meta-test proving the copied detectors flag/pass the
  known fixtures); add npm script `detector:acceptance`.
- **ITEM-5**: Copy `gen-crop-review-manifests.mjs` + `docs/DEFECT_TAXONOMY.md` (the
  vision-crop rubric) into desktop; add npm script `gen:crop-review`.
- **ITEM-6**: Add the desktop overlay-registry gate: copy `gen-overlay-registry.mjs`
  (auto-adapt via `__dirname`) into `desktop/ui/scripts/`; add
  `desktop/ui/src/dev/gallery/overlays.tsx` (desktop overlay manifest) +
  `overlay-allowlist.json`; add `gen:overlay-registry` + `check:overlay-registry`
  npm scripts; chain `check:overlay-registry` into desktop `check`.
- **ITEM-7**: Determine chrome/overlay auditability and audit whatever renders.
  (Reality per DRIFT-1.3: `window`/`file-dialog`/`desktop-base` have NO DOM
  (module/store-only, native-OS) → nothing to render; `updater` `UpdateBanner` +
  `layouts` app-layout are Tauri/shell-coupled and built from already-audited
  shared kit — not fragile-mocked into the gallery. The desktop-only ROUTE
  surfaces (about/remote-access/host-mount/tunnel-magic) ARE in the page gallery
  and are the audited chrome. Determination recorded in `DESKTOP_UI_FINDINGS.md`.)
- **ITEM-8**: RUN the audit (geometry + runtime-health + affordance + a vision crop
  pass) against the desktop-only surfaces; capture consolidated findings to
  `/data/pbya/ziee/tmp/vr-ledger/DESKTOP_UI_FINDINGS.md`.
- **ITEM-9**: FIX the desktop-only GEOMETRY/build defects the audit surfaces using
  the detectors + `DEFECT_TAXONOMY.md` as the rubric. (Real defect found + fixed:
  F1 — the desktop `testid-unique` plugin diverged from web (missing the
  `DefectRepro.tsx` exemption) and BROKE the desktop gallery build. Non-gating
  MEDIUM geometry findings (G5 tap-target, I1 kit-Switch hit-test) are triaged
  benign shared-kit patterns — DEC-7.)
- **ITEM-10**: FIX the desktop-only RUNTIME-HEALTH HIGH findings (contrast,
  a11y-name, console/page/request errors). Audit result: 0 gating HIGH on the
  route surfaces; the F1 plugin fix additionally removes the build-time page-error
  that was aborting the gallery. MEDIUM console-errors are intentional error-state
  cassette logging (triaged benign).
- **ITEM-11**: FIX the desktop-only AFFORDANCE findings. Audit result: 0 affordance
  gaps (the affordance matrix targets deep-chat states, of which desktop has none);
  existing desktop-only specs (remote-access/host-mount) remain green.

## Files to touch

- `src-app/desktop/ui/package.json` — `check` chain + new script entries (ITEM-1..6).
- `src-app/desktop/ui/scripts/gallery-geometry-audit.mjs` (new, copy) — ITEM-2.
- `src-app/desktop/ui/scripts/affordance-audit.mjs` (new, copy) — ITEM-3.
- `src-app/desktop/ui/scripts/affordance-audit-allowlist.json` (new, copy) — ITEM-3.
- `src-app/desktop/ui/scripts/detector-acceptance.mjs` (new, copy) — ITEM-4.
- `src-app/desktop/ui/scripts/gen-crop-review-manifests.mjs` (new, copy) — ITEM-5.
- `src-app/desktop/ui/scripts/gen-overlay-registry.mjs` (new, copy) — ITEM-6.
- `src-app/desktop/ui/src/dev/gallery/geometry-allowlist.json` (new) — ITEM-2.
- `src-app/desktop/ui/src/dev/gallery/__detector_fixtures__/` (new, copy) — ITEM-4.
- `src-app/desktop/ui/src/dev/gallery/DEFECT_TAXONOMY.md` (new, copy into gallery
  docs path the crop script reads: `docs/DEFECT_TAXONOMY.md` relative to ui root) — ITEM-5.
- `src-app/desktop/ui/src/dev/gallery/overlays.tsx` (new) — ITEM-6.
- `src-app/desktop/ui/src/dev/gallery/overlay-allowlist.json` (new) — ITEM-6.
- `src-app/desktop/ui/src/dev/gallery/stories/` + `story` wiring in `main.tsx`/`pages`
  for chrome surfaces — ITEM-7.
- `src-app/desktop/ui/plugins/vite-plugin-testid-unique.js` — F1 fix (port the
  `DefectRepro.tsx` exemption from the web plugin) — ITEM-9/10.
- `src-app/desktop/ui/tests/e2e/gallery-desktop-runtime.spec.ts` (new) — e2e
  verification of ITEM-7..12.
- `/data/pbya/ziee/tmp/vr-ledger/DESKTOP_UI_FINDINGS.md` (out-of-tree ledger) — ITEM-8.

## Patterns to follow

- **Guardrail reuse in package.json** ([[feedback_match_existing_patterns]]): copy
  the exact `../../ui/scripts/<x>.mjs` reference shape desktop already uses for
  `lint:colors` / `lint:settings-field` / `lint:logical-direction` /
  `lint:tooltip-placement` (ITEM-1). For the `check` chain ordering, mirror
  server-ui's `check` order.
- **Copied gallery-relative script** (ITEM-2..6): mirror desktop's existing
  self-owned copies — `desktop/ui/scripts/gen-state-matrix.mjs`,
  `gen-gallery-coverage.mjs`, `gen-crawl-cassette.mjs` — which are near-verbatim
  copies of the server-ui versions that resolve via `__dirname`. Keep the copies
  byte-faithful to the source except where a desktop-specific path/comment differs,
  so future drift is a trivial diff.
- **Desktop gate:ui staging** (ITEM-2,3): mirror server-ui `gate-ui.mjs`'s geometry
  + runtime staging structure; desktop's existing `gate-ui.mjs` already has the
  runtime + coverage stages to slot alongside.
- **Gallery overlays manifest** (ITEM-6,7): mirror server-ui
  `ui/src/dev/gallery/overlays.tsx` + `overlay-registry.generated.json` shape, and
  the story wiring in `ui/src/dev/gallery/{stories,main.tsx}`.
- **Desktop-only surface fixes** (ITEM-9..11): match the token/spacing conventions
  in `DESIGN_SYSTEM.md` and the `Field`-not-raw-gap settings rule; mirror the
  already-audited server-ui settings pages (e.g. `SettingsPageContainer`,
  `McpServerCard`) per [[feedback_match_settings_card_style]]. Use the detectors +
  `DEFECT_TAXONOMY.md` as the defect rubric.

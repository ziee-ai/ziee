# TEST_RESULTS — showcase-modular-seed (Phase 8)

> **Re-verified on the MERGE with origin/main tip f60683384** (56 commits synced).
> Main added 5 new seeded surfaces (`seeded-recent-convos-{loaded,error,loading-more}`,
> `seeded-conversation-list-long{,-narrow}`) to the central `seededSurfaces.tsx` this
> branch aggregatorized + renamed `ChatHistory` fields — all re-homed into
> `chat/gallery.tsx` (baseline now 163). Both workspaces' `npm run check` PASS; all
> generated files regenerated; `gate:ui` runtime canary **181/181 (ui) + 47/47
> (desktop)** runtime-clean; seed-parity + newly-seeded + gap e2e all green; B6 strip
> test passes (seed gate reads the permanent `GALLERY_SEED_EXCEPTIONS.md`, not
> `.lifecycle`). Details below reflect the merged tree.


Diff touches both frontend workspaces (`src-app/ui`, `src-app/desktop/ui`), no
backend → the frontend gate chain applies to BOTH. All Phase-3 TEST-IDs + the
required `npm run check` + boot/runtime canary lines below.

## Unit (node --test)
- **TEST-1**: PASS — `computeSeedDrift` MISSING detection.
- **TEST-2**: PASS — STALE_ALLOWLIST GC.
- **TEST-3**: PASS — `hasUserSurface` route/slot detection (+ commented-route strip).
- **TEST-4**: PASS — `parseSeedExceptions` reason + closed-sign-off.
- **TEST-15**: PASS — a `crawlOnly` module counts as HAS_SEED.
  (TEST-1/2/3/4/15 in `scripts/gen-gallery-seed-registry.test.mjs` — 14 assertions.)
- **TEST-5**: PASS — `extractWiredSurfaces` reads per-module `gallery.tsx` (`scripts/gen-overlay-registry.test.mjs`, 3 assertions).
- **TEST-6**: PASS — `mergeModuleCassettes` merge + collision-throw (`support/registry.test.ts`).
- **TEST-7**: PASS — `assertUniqueSlugs` throws on duplicate.
- **TEST-8**: PASS — `support` durability helpers (`support/index.test.ts`).
  (TEST-6/7/8 — 9 assertions via `node-test-loader`.)

## Integration
- **TEST-14**: PASS — `check-gallery-prod-exclusion.mjs --build`: fresh prod build
  (491 JS assets), runtime marker `ZIEE_GALLERY_SEED_MARKER` ABSENT — the whole
  gallery is tree-shaken out of prod (the dev-gallery lazy import is DEV-gated).
- **TEST-16**: PASS — desktop `npm run check` green (incl. `check:gallery-seed-registry --src src`).
- **TEST-20**: PASS — ui `npm run check` green (incl. `check:gallery-seed-registry`, 39 modules, 36 seeded).

## E2E (Playwright, backend-free `/gallery.html`)
- **TEST-9**: PASS — `gallery-seed-parity.spec.ts`: `listAllSurfaces()` ⊇ the committed 155-slug baseline, no dup.
- **TEST-10**: PASS — `gallery-newly-seeded.spec.ts`: the 5 previously-empty modules render POPULATED (asserts the `[data-gallery-frame]` subtree, not chrome).
- **TEST-11**: PASS — `gallery-newly-seeded.spec.ts`: the 3 newly-wired overlays render OPEN.
- **TEST-12**: PASS — `gallery-gap-seed.spec.ts`: app setup / `/settings/sessions` / sandbox rootfs / `/files/:fileId` render populated.
- **TEST-13**: PASS — runtime-health over ALL surfaces: 176/176 runtime-clean (via `gate:ui`, below).
- **TEST-17**: PASS — desktop `gallery-desktop-seed.spec.ts`: desktop-only `settings-host-mount` (own seed) + shared `settings-js-tool` (cross-workspace `MODULE_CASSETTE`, NOT crawl-covered) both render populated.

## Required gate lines
- `npm run check (ui): PASS`
- `npm run check (desktop/ui): PASS`
- `gate:ui (ui): PASS` — `gate:ui --skip-visual` = tsc + lint + runtime-health, **181/181 surfaces runtime-clean** (post-merge; 176 + 5 new chat surfaces).
- `gate:ui (desktop/ui): PASS` — **47/47 surfaces runtime-clean**.

## Pre-existing failures explicitly NOT caused by this feature (evidence)
The full `gate:ui` (with the visual layer) has TWO pre-existing failure classes,
both verified to fail IDENTICALLY on `origin/main` (e2b5bba) — this feature is a
VERBATIM migration + additive seed, so it introduces none of them:
1. **Runtime** — 4 product-component findings (`overlay-provider-api-key-modal`
   useNavigate-outside-Router crash; `seeded-llm-models-loading` Rules-of-Hooks
   crash; the deliberate `seeded-s3-group-widget-error` 500; a transparent-text
   contrast on `deep-chat-right-panel-file`). Documented in `runtime-baseline.js`
   with per-finding `match` scoping + origin/main evidence. (This feature actually
   FIXES `settings-voice` + `projects`, which fail on main.)
2. **Visual (Layer A)** — `gallery-section-mermaid-block` overflows 20.8px at
   390px — a KIT-STORY (mermaid) layout issue, not a module seed surface; NOT
   baselined here (out of this feature's scope). Layer B pixel regression
   (`VISUAL_SNAPSHOTS=1`) is not run (no new blessed baselines; no visual-design
   change in this feature).
3. **Env note** — KaTeX font `@fs` 403s appeared only because this worktree's
   `node_modules` is SYMLINKED to the main repo (outside the worktree root → vite
   denies); they are documented dev-server harness noise and are now correctly
   subtracted by `gate-ui.mjs` (aligned with `runtime-health.mjs`). Absent on a
   normal checkout / CI.

Full logs: `/data/pbya/ziee/tmp/lifecycle-logs/` (gate:ui runs) + the gallery
`RUNTIME_FINDINGS.{md,jsonl}`.

# PLAN_AUDIT — showcase-modular-seed (Phase 2)

Audited the plan against the real codebase (not on paper). Key couplings probed:
which scripts/consumers read the central gallery files by path, the glob relative
depth, prod-build exclusion, and slug/coverage invariants.

## Breakage risk
- **Overlay-registry gate path-coupling (found, mitigated → ITEM-18).**
  `gen-overlay-registry.mjs:169-173` regexes ONLY `overlays.tsx` for `surface:'…'`
  to compute WIRED overlays. Moving overlay entries to per-module `gallery.tsx`
  makes it find zero → false-fail. **ITEM-18** broadens `getWiredSurfaces()` to scan
  `src/modules/**/gallery.tsx`. This is the ONLY script that path-couples to a
  central gallery file (swept `scripts/` for `overlays.tsx|deepStates.tsx|
  seededSurfaces|fixtures/index|seeded/shard` — only `gen-overlay-registry.mjs`).
- **Other gates are SOURCE-driven, not gallery-file-driven — safe.**
  `gen-gallery-coverage.mjs` fs-walks `.tsx` under `modules`+`components/ui`;
  `gen-state-matrix.mjs` ts-morph-walks the same. Neither reads the central gallery
  arrays, so the aggregator refactor does not touch them. `surfaces.ts` /
  `pages.tsx` read the exported arrays (`OVERLAY_ENTRIES` etc.), which the
  aggregators keep exporting — unchanged.
- **Slug / surface-id invariant (regression risk during migration).**
  `coverage.ts` (GALLERY_COVERAGE) + `stateCoverage.ts` (STATE_COVERAGE) key on
  surface ids + slugs. Migration MUST preserve every slug byte-for-byte (overlay
  `slug`+`surface`, deep `slug`, seeded `slug`). A renamed slug silently drops
  coverage. Mitigation: mechanical move of the exact entry objects; a slug-set
  diff (before/after) asserted in a unit test (TEST budget in Phase 3); the eager
  registry throws on duplicate/missing slug.
- **Eager-glob bundle-leak risk (prod).** `import.meta.glob(...,{eager:true})` pulls
  ALL matched `gallery.tsx` into whatever chunk imports `registry.ts`. Invariant:
  `registry.ts` + the aggregators are imported ONLY from the dev-gallery chunk
  (reached via `main.tsx`/`gallery.html` — NOT a prod build input — and the
  DEV-gated dev-gallery route). Verified today's build has NO custom
  `rollupOptions.input` (prod builds only `index.html`). **ITEM-16** confirms with a
  real `vite build` + asserts no `gallery.tsx` content in the app entry chunk; if it
  leaks, gate the dev-gallery lazy `import()` behind `import.meta.env.DEV` so the
  reference is dropped in prod.
- **Ordering invariant preserved.** Eager glob is synchronous at module-eval, so
  `GALLERY_CASSETTE` is fully assembled before `seed.ts` calls `installMockApi()` →
  `loadModules()` (the existing must-install-before-load contract).
- **Cross-module cassette collision.** Two modules seeding the same endpoint key =
  a real ambiguity. `mergeModuleCassettes` THROWS on duplicate key (dev-only) so it
  surfaces at gallery boot, not as a silent last-wins.
- **Desktop divergence.** Desktop gallery is a hand-copy with its own subset
  `fixtures/` + `loadDesktopModules`. ITEM-15 must glob BOTH shared
  (`../../../../ui/src/modules/**`) and desktop-only (`../../modules/**`) gallery
  files; the desktop `gen-overlay-registry`/coverage gates already walk the desktop
  tree. R2-3: diff desktop gallery changes vs the ui equivalent (no security logic
  here — pure dev harness — but keep them structurally in sync).

## Pattern conformance
- Auto-discovery mirrors `loader.ts`'s `import.meta.glob('./**/module.tsx')` exactly
  (same eager-glob idiom, one directory over). PASS.
- The completeness gate mirrors `gen-override-registry.mjs` (set-difference, committed
  allow-list w/ sign-off, byte-compared manifest, pure `computeDrift`, GC stale-allow).
  PASS — copies an in-tree, just-merged precedent.
- Per-module authoring generalizes the existing seeded-**shard** contract
  (`seeded/shard<N>.tsx` + `helpers.tsx`) from "shard file" to "module `gallery.tsx`".
  The `support/` barrel is the generalized `seeded/helpers.tsx`. PASS.
- `stories/` (kit-component design-system stories) stays central — they are NOT
  module-owned seed; `ModuleGallery.stories?` exists for the rare module story but
  the kit stories don't migrate. Conforms to "mirror the closest precedent".

## Migration collisions
- **No DB migration** (frontend-only) — zero collision with the highest existing
  `00000000000157`.
- **File-level:** main is actively churning `src/dev/gallery/**`. This branch
  rewrites the 4 central aggregator files + retires `seeded/shard1..5`. Real
  merge-conflict surface, but bounded: bulk of NEW code is in per-module
  `gallery.tsx` files main isn't touching. The merge-gate (C2/C4 + staging-merge)
  re-checks against real main at merge time. `package.json` `check` edit is
  append-only (low collision).

## OpenAPI regen
- **None.** No backend types change. Cassette fixtures CONSUME the existing generated
  `@/api-client/types` (`GetResponseType<K>`). If a gap module needs an endpoint
  absent from the spec, that is a stop-and-flag signal, not a regen. Confirmed the 5
  unseeded modules' on-load endpoints already exist in `ApiEndpoints` (they're real
  routes; the crawl simply never recorded them).

## Per-item verdicts
- **ITEM-1** — verdict: PASS — generalizes `seeded/helpers.tsx`; additive shared barrel.
- **ITEM-2** — verdict: PASS — eager-glob mirrors `loader.ts`; collision-throw is new but dev-only + safe.
- **ITEM-3** — verdict: PASS — keeps crawl base + all re-exports; last-wins order preserved.
- **ITEM-4** — verdict: CONCERN — breaks `gen-overlay-registry` until **ITEM-18** lands; sequence ITEM-18 with ITEM-4.
- **ITEM-5** — verdict: PASS — deep is chat-only; aggregator keeps `DeepStateFrame`/`deepStateBySlug`.
- **ITEM-6** — verdict: PASS — retire seeded shards into modules; gallery-local demos → dev-gallery home; keep frame + bySlug exports.
- **ITEM-7** — verdict: PASS — mirrors override gate; B6-safe (all inputs committed product-tree paths).
- **ITEM-8** — verdict: PASS — move 7 fixtures into module cassettes; preserve endpoint keys + resolvers.
- **ITEM-9** — verdict: CONCERN — 44 overlays; MUST preserve `slug`+`surface` exactly (coverage/overlay-registry key on them). Slug-diff test.
- **ITEM-10** — verdict: PASS — 17 deep-states into chat/gallery; conversationIds come from chat-deep fixtures (move together).
- **ITEM-11** — verdict: CONCERN — 94 seeded; largest mechanical move; preserve slugs; the `window.fetch`-shim + action-patch escape hatches move verbatim.
- **ITEM-12** — verdict: PASS — new typed cassette + wire 3 unwired overlays; endpoints exist in spec.
- **ITEM-13** — verdict: PASS — gap fixtures typed literals; each endpoint exists.
- **ITEM-14** — verdict: PASS — 5 infra modules allow-listed w/ structural reason + sign-off (mirror OVERRIDE_EXCEPTIONS.md).
- **ITEM-15** — verdict: CONCERN — desktop glob depth + its own `check` wiring; R2-3 sync; verify desktop `npm run check` green natively.
- **ITEM-16** — verdict: PASS — real `vite build` + chunk inspection; mitigation (DEV-gate the lazy import) on leak.
- **ITEM-17** — verdict: PASS — minimal `crawlOnly:true` markers make ownership uniform; gate accepts them as HAS_SEED.
- **ITEM-18** — verdict: PASS — broadens `getWiredSurfaces()` glob; unit-tested against a per-module fixture.

No `BLOCKED` verdicts. The three CONCERNs (ITEM-4 ordering, ITEM-9/11 slug
preservation, ITEM-15 desktop) are handled by sequencing + the enumerated tests,
not by plan changes beyond ITEM-18 (already amended into PLAN.md).

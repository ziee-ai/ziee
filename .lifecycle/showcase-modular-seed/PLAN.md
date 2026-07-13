# PLAN — showcase-modular-seed

Modularize the dev gallery so each module owns its showcase seed (auto-discovered
like `module.tsx`), add a committed completeness gate that fails when a
surface-bearing module has no registered seed, and seed every module through the
new mechanism. Full design in `DESIGN.md`; survey in `SURVEY.md` + `survey/01..05`.

> **Premise correction (relay to human):** the gallery is NOT "~4/41 seeded". Reality
> today: **24 SEEDED, 2 PARTIAL, 5 UNSEEDED, 5 INFRA-ONLY**, ~7 with rich hand
> fixtures. The real defect is that ALL seed authoring is **centralized** and there is
> **no gate**, so 5 modules (`js-tool`, `knowledge-base`, `notification`, `scheduler`,
> `voice`) silently render empty. The plan targets that.

## Items

### A. Mechanism (per-module registration)
- **ITEM-1**: Add `ModuleGallery` type + a shared `src/dev/gallery/support/` barrel — generalize `seeded/helpers.tsx` (lazyNamed/lazyBound/lazyProps/lazyCompose/holdPatch/holdForever/whenTrue) + re-export the entry types (`OverlayEntry`/`DeepStateEntry`/`SeededSurfaceEntry`/`GalleryStory`) so per-module `gallery.tsx` imports ONLY from `support/`.
- **ITEM-2**: Add `support/registry.ts` — eager `import.meta.glob('../../../modules/**/gallery.{ts,tsx}')` collector; `mergeModuleCassettes` (crawl-base-first, per-module-wins, THROW on duplicate endpoint key across modules); slug-uniqueness assertion across overlays/deep/seeded (dev-throws on collision, replacing the shard slug-prefix convention).
- **ITEM-3**: Convert `fixtures/index.ts` → `GALLERY_CASSETTE = { ...crawlCassette, ...mergeModuleCassettes(...) }`; keep `crawl.generated.ts` + the recorder untouched (shared base). Re-export `adminUser`/`adminMe`/`adminPermissions`/`showcaseConversationIds` unchanged.
- **ITEM-4**: Convert `overlays.tsx` → thin aggregator: `OVERLAY_ENTRIES = MODULE_GALLERIES.flatMap(g => g.overlays ?? [])`; keep `overlayBySlug`, `OVERLAY_SLUGS`, `WIRED_OVERLAY_SURFACES` exports (so `overlay-registry` gate + `surfaces.ts` are unchanged).
- **ITEM-18**: Update `scripts/gen-overlay-registry.mjs::getWiredSurfaces()` — it currently regexes ONLY `overlays.tsx` for `surface:` fields (`gen-overlay-registry.mjs:169-173`). Once overlay entries move to per-module `gallery.tsx`, that regex finds zero → false-fail. Broaden it to scan `src/modules/**/gallery.tsx` (+ the residual `overlays.tsx` during transition) for `surface:` literals. (Discovered in Phase-2 audit; the ONLY existing gate that path-couples to a central gallery file.)
- **ITEM-5**: Convert `deepStates.tsx` → thin aggregator over per-module `deepStates`; keep `DeepStateFrame`, `deepStateBySlug`, `DEEP_STATE_SLUGS`.
- **ITEM-6**: Convert `seededSurfaces.tsx` → thin aggregator; retire `seeded/shard1..5.tsx` (their entries move to owning modules); keep `SeededSurfaceFrame`, `seededSurfaceBySlug`, `SEEDED_SURFACE_SLUGS`. Gallery-local demo components (`DefectRepro`, `TableDemos`, `MessageListLongDemo`) + their entries move under a `dev-gallery`-owned home (`src/modules/dev-gallery/gallery.tsx` or `support/local/`).

### B. The completeness gate (mirror gen-override-registry)
- **ITEM-7**: Add `scripts/gen-gallery-seed-registry.mjs` (`gen:` + `--check`): pure `hasUserSurface(moduleTsxSrc)` + `hasSeed(dir)` + `computeSeedDrift(modules, allowlist)`; MISSING + STALE_ALLOWLIST fail; emit byte-compared `src/dev/gallery/GALLERY_SEED_MANIFEST.md`; read committed allow-list `src/dev/gallery/GALLERY_SEED_EXCEPTIONS.md` (`- NO-SEED: <module> — <reason> [approved: …]`). Add `check:gallery-seed-registry` to `npm run check` (last). B6-verified against a `.lifecycle`-stripped tree.
- **ITEM-14**: Author `GALLERY_SEED_EXCEPTIONS.md` with the 5 INFRA-ONLY modules (config-client, dev-gallery‡, layouts, router, settings) — each with a structural reason + sign-off. (‡ dev-gallery owns the local demos, so it may instead get a `gallery.tsx`; decided in Phase 4.)

### C. Migration of existing central seed  *(scope-gated: DEC-A)*
- **ITEM-8**: Migrate the 7 hand fixtures (auth, chat+chat-deep, citations, llm-providers, project-deep, workflow, skills) into their modules' `gallery.tsx` `cassette`.
- **ITEM-9**: Migrate all 44 overlays into owning modules' `gallery.tsx` `overlays` (per `survey/03` slug→module table). Kit dialog-host overlays → `components/ui` home.
- **ITEM-10**: Migrate 17 deep-states into `chat`'s `gallery.tsx` `deepStates`.
- **ITEM-11**: Migrate 94 seeded surfaces into owning modules' `gallery.tsx` `seeded`.

### D. Seed the gaps (new coverage)
- **ITEM-12**: Seed the 5 UNSEEDED modules via their new `gallery.tsx`: cassette for on-load GETs + list/detail data; wire the 3 unwired overlays (KnowledgeBaseFormDrawer, ScheduledTaskFormDrawer, UploadModelDrawer); add a `kb_source` right-panel seeded surface for knowledge-base.
- **ITEM-13**: Fill the noted gaps: `App.getSetupStatus` (app), onboarding guide steps, `Auth.getSessionSettings` (/settings/sessions), `CodeSandbox.listRootfsVersions`, `File.get` for `/files/:fileId`.
- **ITEM-17**: For every other surface-bearing module with no bespoke entries, add a minimal `gallery.tsx` (`crawlOnly: true` marker) so per-module ownership is uniform and the gate passes.

### E. Desktop parity  *(scope-gated: DEC-B)*
- **ITEM-15**: Desktop gallery aggregators glob shared (`../../../../ui/src/modules/**`) + desktop-only (`../../modules/**`) `gallery.tsx`; seed the 5 desktop-only surface-bearing modules (host-mount, remote-access, tunnel-auth, updater, window); run `check:gallery-seed-registry` in the desktop workspace; keep desktop `npm run check` green.

### F. Safety
- **ITEM-16**: Verify prod app build excludes `gallery.tsx` (glob lives only in the gallery entry; `module.tsx` never imports `gallery.tsx`) — no prod bundle bloat, gallery never ships. Confirm the standalone `gallery.html` is dev-only.

## Files to touch
- **New (per-module):** `src-app/ui/src/modules/<X>/gallery.tsx` (~30 modules).
- **New (infra):** `src-app/ui/src/dev/gallery/support/{types.ts,registry.ts,index.ts}`,
  `scripts/gen-gallery-seed-registry.mjs`, `src/dev/gallery/GALLERY_SEED_EXCEPTIONS.md`,
  `src/dev/gallery/GALLERY_SEED_MANIFEST.md` (generated), tests under
  `scripts/*.test.mjs` + `src/dev/gallery/*.test.ts`.
- **Refactor (central → aggregator):** `src/dev/gallery/fixtures/index.ts`,
  `overlays.tsx`, `deepStates.tsx`, `seededSurfaces.tsx`; retire `seeded/shard1..5.tsx`
  (+ `stories/shard*.story.tsx`); move `seeded/helpers.tsx` → `support/`.
- **Wire:** `src-app/ui/package.json` (`gen:`/`check:gallery-seed-registry`, append to `check`).
- **Desktop (DEC-B):** mirror in `src-app/desktop/ui/src/dev/gallery/*` + `package.json`;
  new `src-app/desktop/ui/src/modules/<desktop-only>/gallery.tsx`.
- **Untouched:** `mockApi.ts`, `surfaces.ts`, `pages.tsx`, `interactions.ts`,
  `crawl.generated.ts`, `record-gallery-fixtures.mjs`, coverage/state gates, all backend.

## Patterns to follow
- **Auto-discovery** → `src/modules/loader.ts` (`import.meta.glob('./**/module.tsx',{eager})`) + the router module's declaration-merge + `onModuleRegister` harvest (`modules/router/{types.ts,module.tsx}`).
- **The gate** → `scripts/gen-override-registry.mjs` + `desktop/ui/OVERRIDE_EXCEPTIONS.md` (set-difference, committed allow-list w/ sign-off, byte-compared manifest, pure `computeDrift`, GC stale-allow) — copy its shape exactly.
- **Per-module authoring contract** → the existing seeded **shard** contract (`seeded/shard<N>.tsx` + `seeded/helpers.tsx`) generalized from "shard file" to "module `gallery.tsx`".
- **Cassette typing** → `mockApi.ts` `Cassette`/`CassetteEntry<K>` (typed vs `@/api-client/types`).
- **Settings cards for the gap modules** → mirror the closest sibling settings page per `feedback_match_settings_card_style` (already the modules' own pages — we only SEED them, not restyle).

## UI-surface plan checklist
This feature adds NO new product UI — it seeds EXISTING module surfaces into the
dev gallery. So the checklist applies to the gallery's own rendering, not new pages:
- **Precedent** — each `gallery.tsx` mirrors the seeded-shard authoring pattern; the
  gate mirrors `gen-override-registry`. No bespoke UI invented.
- **Scale/cardinality** — the runtime glob is eager over ~35 small files (bounded);
  the manifest lists ≤44 modules. No unbounded list.
- **Device size** — the gallery already renders a narrow-viewport state; newly-seeded
  surfaces inherit the existing 390px/desktop coverage via `state-matrix`.
- **Progress / input economy / multi-instance / platform affordances** — N/A (no new
  product surface); the seeded surfaces are the modules' real ones, unchanged.
- **JTBD** — the "user" is a coding agent/reviewer running `gate:ui`/`runtime-health`:
  they want EVERY module's page/overlay/component to render populated across states so
  visual review + regression catch defects. Success = 0 UNSEEDED, gate green, every
  newly-seeded surface passes runtime-health (no console error / crash / contrast fail).

## Scope decisions — LOCKED by human at plan time (AskUserQuestion 2026-07-13)
- **DEC-A** = **FULL migration now.** All 155 central entries → per-module `gallery.tsx`;
  central files become pure aggregators. ITEM-8..11 are IN scope (no descope).
- **DEC-B** = **Include desktop now.** ITEM-15 in scope: desktop aggregators glob
  shared + desktop-only; seed 5 desktop-only modules; gate runs in desktop workspace.
- **DEC-C** = **Hand-authored typed literals** for all new gap seed (no new recorder
  arms); `record-gallery-fixtures.mjs` untouched.

These three carry into DECISIONS.md (Phase 4) as resolved `### DEC-*` entries.

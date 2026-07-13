/**
 * ZIEE_GALLERY_SEED_MARKER — the runtime registry that auto-discovers every
 * per-module `src/modules/<X>/gallery.tsx` and assembles the four surface
 * classes + the mock-API cassette the gallery replays.
 *
 * Discovery mirrors the module loader's eager glob of every `module.tsx`, one
 * directory over, EAGER so the cassette is fully assembled synchronously at
 * module-eval — BEFORE `seed.ts` calls `installMockApi()` then `loadModules()`.
 *
 * The sentinel comment above is asserted-absent from the prod app bundle by
 * `scripts/check-gallery-prod-exclusion.mjs`: this module (and everything it
 * eager-imports) is reachable ONLY from the dev-gallery chunk, never app-main.
 *
 * Pure logic (merge/assert) lives in `registry-core.ts` so it unit-tests without
 * vite; this file only adds the `import.meta.glob` discovery on top.
 */
import type { Cassette } from '../mockApi'
import type {
  DeepStateEntry,
  ModuleGallery,
  OverlayEntry,
  SeededSurfaceEntry,
} from './types'
import type { GalleryStory } from '../story'
import {
  type DiscoveredGallery,
  assertUniqueSlugs,
  mergeModuleCassettes,
  moduleNameFromPath,
} from './registry-core'

export type { DiscoveredGallery } from './registry-core'
export {
  assertUniqueSlugs,
  mergeModuleCassettes,
  moduleNameFromPath,
} from './registry-core'

/**
 * Eager-glob every module's `gallery.{ts,tsx}`. Relative to THIS file
 * (`src/dev/gallery/support/`): `../../../modules` resolves to `src/modules`.
 */
export function collectModuleGalleries(): DiscoveredGallery[] {
  const mods = import.meta.glob<{ gallery?: ModuleGallery }>(
    '../../../modules/**/gallery.{ts,tsx}',
    { eager: true },
  )
  return Object.entries(mods)
    .map(([path, m]) => ({ module: moduleNameFromPath(path), gallery: m.gallery }))
    .filter((x): x is DiscoveredGallery => Boolean(x.gallery))
    .sort((a, b) => a.module.localeCompare(b.module))
}

// ── Discovered singletons (assembled once at module-eval) ────────────────────
const DISCOVERED = collectModuleGalleries()
assertUniqueSlugs(DISCOVERED)

export const MODULE_GALLERIES: DiscoveredGallery[] = DISCOVERED
export const MODULE_CASSETTE: Cassette = mergeModuleCassettes(DISCOVERED)
export const OVERLAY_ENTRIES: OverlayEntry[] = DISCOVERED.flatMap(
  g => g.gallery.overlays ?? [],
)
export const DEEP_STATE_ENTRIES: DeepStateEntry[] = DISCOVERED.flatMap(
  g => g.gallery.deepStates ?? [],
)
export const SEEDED_SURFACE_ENTRIES: SeededSurfaceEntry[] = DISCOVERED.flatMap(
  g => g.gallery.seeded ?? [],
)
export const MODULE_STORIES: GalleryStory[] = DISCOVERED.flatMap(
  g => g.gallery.stories ?? [],
)

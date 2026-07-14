/**
 * ZIEE_GALLERY_SEED_MARKER — the app-side surface discovery. `import.meta.glob`
 * is Vite-only and cannot cross the `@ziee/gallery` package boundary, so the glob
 * stays here (mirroring `modules/loader.ts`, one dir over, EAGER so the cassette
 * is assembled synchronously at module-eval). The discovered galleries are
 * INJECTED into the framework via `mountGallery({ discoverGalleries })`; the pure
 * merge/assert (`mergeModuleCassettes` / `assertUniqueSlugs`) lives in the package.
 *
 * `MODULE_CASSETTE` / `MODULE_GALLERIES` stay exported so the shared crawl
 * fixtures barrel + the desktop cross-workspace cassette bridge keep working.
 */
import {
  type DiscoveredGallery,
  mergeModuleCassettes,
  moduleNameFromPath,
} from '@ziee/gallery'
import type { Cassette, ModuleGallery } from './types'

export type { DiscoveredGallery } from '@ziee/gallery'
export { assertUniqueSlugs, mergeModuleCassettes, moduleNameFromPath } from '@ziee/gallery'

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

export const MODULE_GALLERIES: DiscoveredGallery[] = DISCOVERED
export const MODULE_CASSETTE = mergeModuleCassettes(DISCOVERED) as Cassette

/** Injected into `mountGallery` — discovery stays app-side (Vite-only glob). */
export const discoverGalleries = (): DiscoveredGallery[] => DISCOVERED

/**
 * Desktop dev-gallery cross-workspace surface discovery — the desktop's
 * `discoverGalleries()` injected into `@ziee/gallery`'s `mountGallery({ ... })`.
 *
 * The desktop app SHARES the web-core modules (via the `@/` override plugin) and
 * adds desktop-only modules. So the desktop gallery's surfaces are assembled from:
 *   1. the SHARED web modules' merged per-module cassette
 *      (`@/dev/gallery/support/registry` resolves — via the override fallback — to
 *      the web workspace's registry, whose eager glob is anchored to
 *      `src-app/ui/src/modules`), and
 *   2. the DESKTOP-only modules' per-module `gallery.tsx` (globbed here).
 *
 * `import.meta.glob` is Vite-only (it cannot cross the `@ziee/gallery` package
 * boundary), so discovery stays app-side and is injected; the pure merge/assert
 * (`mergeModuleCassettes` / `assertUniqueSlugs`) comes straight from the package.
 *
 * PAGE-FOCUSED: the desktop canvas renders module PAGES only — kit-component
 * stories + interaction-only overlay/deep/seeded surfaces live in the web
 * workspace. So the shared web cassette is inherited (shared pages render
 * populated) but the web overlay/deep/seeded/story ENTRIES are intentionally NOT
 * pulled in. Desktop-only modules keep their full gallery (their cassette is
 * folded into the merged blob; any future non-cassette surfaces flow through).
 */
import {
  type DiscoveredGallery,
  assertUniqueSlugs,
  mergeModuleCassettes,
  moduleNameFromPath,
} from '@ziee/gallery'
import type { Cassette } from './mockApi'
import type { ModuleGallery } from '@/dev/gallery/support'
import { MODULE_CASSETTE as SHARED_CASSETTE } from '@/dev/gallery/support/registry'

// Desktop-only module seeds (same-workspace glob).
const desktopMods = import.meta.glob<{ gallery?: ModuleGallery }>(
  '../../modules/**/gallery.{ts,tsx}',
  { eager: true },
)
const DESKTOP_GALLERIES: DiscoveredGallery[] = Object.entries(desktopMods)
  .map(([path, m]) => ({ module: moduleNameFromPath(path), gallery: m.gallery }))
  .filter((x): x is DiscoveredGallery => Boolean(x.gallery))
  .sort((a, b) => a.module.localeCompare(b.module))

assertUniqueSlugs(DESKTOP_GALLERIES)

// Shared web cassette first; desktop-only entries win on any (rare) key overlap.
export const MODULE_CASSETTE: Cassette = {
  ...(SHARED_CASSETTE as Cassette),
  ...(mergeModuleCassettes(DESKTOP_GALLERIES) as Cassette),
}

/**
 * The desktop's injected surface discovery. Returns:
 *   - ONE synthetic entry carrying the cross-workspace merged CASSETTE (so the
 *     package's re-merge sees a single, collision-free source, preserving the
 *     "desktop wins on overlap" semantics `mergeModuleCassettes` would otherwise
 *     throw on), and
 *   - the desktop-only galleries with their cassette OMITTED (already folded into
 *     the synthetic entry) so their non-cassette surfaces still flow through.
 */
export function discoverGalleries(): DiscoveredGallery[] {
  return [
    { module: '__desktop_merged_cassette__', gallery: { cassette: MODULE_CASSETTE } },
    ...DESKTOP_GALLERIES.map(g => ({
      module: g.module,
      gallery: { ...g.gallery, cassette: undefined },
    })),
  ]
}

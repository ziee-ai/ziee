/**
 * Desktop dev-gallery per-module seed registry.
 *
 * The desktop app SHARES the web-core modules (via the `@/` override plugin) and
 * adds desktop-only modules. So the desktop gallery's cassette is assembled from:
 *   1. the SHARED web modules' per-module seed (`@/dev/gallery/support/registry`
 *      resolves — via the override fallback — to the web workspace's registry,
 *      whose eager glob is anchored to `src-app/ui/src/modules`), and
 *   2. the DESKTOP-only modules' per-module `gallery.tsx` (globbed here).
 *
 * Mirrors the web `support/registry.ts`; pure merge/assert logic is reused from
 * the shared `registry-core`.
 */
import type { Cassette } from './mockApi'
import type { ModuleGallery } from '@/dev/gallery/support'
import {
  type DiscoveredGallery,
  assertUniqueSlugs,
  mergeModuleCassettes,
  moduleNameFromPath,
} from '@/dev/gallery/support/registry-core'
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
// NOTE: the desktop gallery is PAGE-focused — it does not enumerate the web
// overlay/deep/seeded classes — so only the CASSETTE is inherited (shared pages
// render populated); the web overlay ENTRIES are intentionally not pulled in.
export const MODULE_CASSETTE: Cassette = {
  ...(SHARED_CASSETTE as Cassette),
  ...(mergeModuleCassettes(DESKTOP_GALLERIES) as Cassette),
}

/**
 * ZIEE_GALLERY_SEED_MARKER — the runtime registry that auto-discovers every
 * per-module `src/modules/<X>/gallery.tsx` and assembles the four surface
 * classes + the mock-API cassette the gallery replays.
 *
 * Discovery mirrors the module loader's eager glob of every `module.tsx`, one
 * directory over, EAGER so the cassette is fully assembled synchronously at
 * module-eval — BEFORE `seed.ts` calls `installMockApi()` → `loadModules()`.
 *
 * The sentinel comment above is asserted-absent from the prod app bundle by
 * `scripts/check-gallery-prod-exclusion.mjs`: this module (and everything it
 * eager-imports) is reachable ONLY from the dev-gallery chunk, never app-main.
 *
 * The pure functions (`mergeModuleCassettes`, `assertUniqueSlugs`) take an
 * explicit gallery list so they unit-test without vite.
 */
import type { Cassette } from '../mockApi'
import type {
  DeepStateEntry,
  ModuleGallery,
  OverlayEntry,
  SeededSurfaceEntry,
} from './types'
import type { GalleryStory } from '../story'

/** One discovered per-module seed: its module name + the exported `gallery`. */
export interface DiscoveredGallery {
  module: string
  gallery: ModuleGallery
}

/** `../../../modules/foo/gallery.tsx` → `foo`. */
export function moduleNameFromPath(path: string): string {
  const m = path.match(/modules\/([^/]+)\/gallery\.(?:ts|tsx)$/)
  return m ? m[1] : path
}

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

/**
 * Merge per-module cassettes into one. Each module seeds only ITS OWN endpoints,
 * so a duplicate endpoint key across two modules is a real ambiguity → THROW
 * (dev-only, surfaces at gallery boot). The shared crawl base is applied
 * separately (and always overridden) in `fixtures/index.ts`.
 */
export function mergeModuleCassettes(galleries: DiscoveredGallery[]): Cassette {
  const out: Cassette = {}
  const owner: Record<string, string> = {}
  for (const { module, gallery } of galleries) {
    if (!gallery.cassette) continue
    for (const key of Object.keys(gallery.cassette) as (keyof Cassette)[]) {
      if (owner[key as string]) {
        throw new Error(
          `[gallery] cassette collision on "${String(key)}": both "${owner[key as string]}" and "${module}" seed it. One module must own each endpoint.`,
        )
      }
      owner[key as string] = module
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      ;(out as any)[key] = (gallery.cassette as any)[key]
    }
  }
  return out
}

/** Throw on a duplicate slug across ALL surface classes (was the shard slug-prefix convention). */
export function assertUniqueSlugs(galleries: DiscoveredGallery[]): void {
  const seen: Record<string, string> = {}
  const check = (slug: string, module: string, cls: string) => {
    const prev = seen[slug]
    if (prev) {
      throw new Error(
        `[gallery] duplicate surface slug "${slug}" — ${prev} and ${module}/${cls}. Slugs must be unique across overlays/deep/seeded.`,
      )
    }
    seen[slug] = `${module}/${cls}`
  }
  for (const { module, gallery } of galleries) {
    for (const o of gallery.overlays ?? []) check(o.slug, module, 'overlay')
    for (const d of gallery.deepStates ?? []) check(d.slug, module, 'deep')
    for (const s of gallery.seeded ?? []) check(s.slug, module, 'seeded')
  }
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

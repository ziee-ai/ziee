/**
 * SINGLE SOURCE of gallery-surface enumeration (desktop mirror).
 *
 * The desktop gallery is PAGE-focused — kit component stories + interaction-only
 * overlay/deep/seeded surfaces live in the web workspace — so the only class with
 * entries here is `pages` (read from the browse DOM). The other three classes are
 * empty, but the SHAPE matches the web workspace's `surfaces.ts` so the shared
 * capture/coverage tooling consumes both identically and can never silently skip
 * a class if one is added here later.
 */
export interface GallerySurfaceClasses {
  pages: string[]
  overlays: string[]
  deep: string[]
  seeded: string[]
}

/** No interaction-only surfaces on the desktop canvas (see module note above). */
export const OVERLAY_SLUGS: string[] = []
export const DEEP_SLUGS: string[] = []
export const SEEDED_SLUGS: string[] = []

/** Enumerate every gallery surface class. Call on the browse canvas. */
export function listAllSurfaces(): GallerySurfaceClasses {
  const special = new Set([...OVERLAY_SLUGS, ...DEEP_SLUGS, ...SEEDED_SLUGS])
  const pages =
    typeof document !== 'undefined'
      ? Array.from(
          document.querySelectorAll('[data-testid^="gallery-page-"]'),
        )
          .map(el =>
            (el.getAttribute('data-testid') || '').replace('gallery-page-', ''),
          )
          .filter(id => id && !special.has(id))
      : []
  return {
    pages: [...new Set(pages)],
    overlays: OVERLAY_SLUGS,
    deep: DEEP_SLUGS,
    seeded: SEEDED_SLUGS,
  }
}

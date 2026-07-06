/**
 * SINGLE SOURCE of gallery-surface enumeration.
 *
 * The gallery has FOUR surface classes and they render through different
 * channels:
 *   - **pages**   — real module routes, present on the browse canvas (read from
 *                   the rendered DOM: `[data-testid^="gallery-page-"]`);
 *   - **overlays**— interaction-only Sheet/Dialog open-states (static list);
 *   - **deep**    — active-conversation deep-states (static list);
 *   - **seeded**  — real components with a mount-time store seed (static list).
 *
 * Only the pages appear on the browse canvas; the other three are driven one per
 * page-load via `?surface=<slug>`. A capture/coverage pass that enumerates ONLY
 * the browse DOM therefore SILENTLY SKIPS the other three classes — the exact
 * gap this module closes. Everything (captures + coverage) enumerates through
 * `listAllSurfaces()` (published on `window.__GALLERY_LIST_ALL_SURFACES__`), so a
 * new surface class can never again be missed by one tool but not another.
 */
import { OVERLAY_ENTRIES } from './overlays'
import { DEEP_STATE_ENTRIES, DEEP_STATE_SLUGS } from './deepStates'
import { SEEDED_SURFACE_ENTRIES, SEEDED_SURFACE_SLUGS } from './seededSurfaces'
import {
  type InteractionManifestEntry,
  buildInteractionManifest,
} from './interactions'

export interface GallerySurfaceClasses {
  /** Data-state pages (browse canvas) — driven via `?surface=&state=`. */
  pages: string[]
  /** Overlay open-states — driven via `?surface=<slug>`. */
  overlays: string[]
  /** Active-conversation deep-states — driven via `?surface=<slug>`. */
  deep: string[]
  /** Seeded real-component surfaces — driven via `?surface=<slug>`. */
  seeded: string[]
  /** Interaction recipes — driven via `?surface=<slug>&interact=<name>`; each is a
   *  post-mount user action (click-to-edit, expand, focus, hover) that renders an
   *  interaction-gated state the mount-only pass never shows. */
  interactions: InteractionManifestEntry[]
}

/** Static (interaction-only) surface slug lists, from the entry arrays. */
export const OVERLAY_SLUGS: string[] = OVERLAY_ENTRIES.map(o => o.slug)
export const DEEP_SLUGS: string[] = DEEP_STATE_SLUGS
export const SEEDED_SLUGS: string[] = SEEDED_SURFACE_SLUGS

/** Flat interaction manifest across all interaction-bearing entry classes. */
export const INTERACTION_MANIFEST: InteractionManifestEntry[] =
  buildInteractionManifest([
    ...OVERLAY_ENTRIES,
    ...DEEP_STATE_ENTRIES,
    ...SEEDED_SURFACE_ENTRIES,
  ])

/**
 * Enumerate EVERY gallery surface across all four classes. Pages are read from
 * the rendered browse DOM (they only exist after the router store populates);
 * the other three are static entry lists. Call this on the browse canvas (no
 * `?surface=`) so the page list is populated.
 */
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
    // De-dup: a slug could appear twice if the browse canvas and a mounted frame
    // coexist in the DOM.
    pages: [...new Set(pages)],
    overlays: OVERLAY_SLUGS,
    deep: DEEP_SLUGS,
    seeded: SEEDED_SLUGS,
    interactions: INTERACTION_MANIFEST,
  }
}

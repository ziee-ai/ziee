/**
 * Overlay open-state aggregator.
 *
 * Overlay entries are now OWNED per-module in `src/modules/<X>/gallery.tsx`
 * (`gallery.overlays`) and auto-discovered by the runtime registry
 * (`support/registry.ts`). This file is the thin aggregator the rest of the
 * gallery (`surfaces.ts`, `pages.tsx`) reads through, keeping the SAME export
 * surface so those consumers are unchanged.
 */
import { OVERLAY_ENTRIES } from './support/registry'
import type { OverlayEntry } from './support/types'

export type { OverlayEntry }
export { OVERLAY_ENTRIES }

/** Surface ids covered with a delivered open-state entry (for the coverage gate). */
export const WIRED_OVERLAY_SURFACES = new Set(OVERLAY_ENTRIES.map(o => o.surface))

export const overlayBySlug = (slug: string) =>
  OVERLAY_ENTRIES.find(o => o.slug === slug)

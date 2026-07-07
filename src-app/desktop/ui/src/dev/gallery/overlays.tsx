/**
 * Gallery overlay manifest (desktop mirror).
 *
 * A "wired-open" overlay entry renders a controlled Dialog/Drawer/Sheet/Popover
 * in its OPEN state on the gallery canvas so the visual/runtime layers can review
 * it standalone. The desktop workspace is PAGE-focused and currently has NO
 * overlay that is cleanly openable via a store/prop on the standard canvas — the
 * single desktop-only overlay host (`ConversationMountsControl`, a composer
 * Popover) is interaction-gated inside the live chat composer and is therefore
 * ALLOW-LISTED in `overlay-allowlist.json` rather than wired open here.
 *
 * The SHAPE matches the web workspace's `overlays.tsx` so the shared
 * `gen-overlay-registry.mjs` gate consumes both identically: it regexes the
 * `surface:` fields below to know which hosts are wired open. Add an entry here
 * (instead of an allow-list reason) whenever a desktop overlay becomes
 * standalone-openable.
 */
import type { ComponentType, ReactNode } from 'react'

export interface OverlayEntry {
  /** Stable slug for the overlay surface (drives the gallery testid). */
  slug: string
  /** The module-relative host surface id, e.g. `modules/foo/components/Bar`. */
  surface: string
  /** Human label. */
  label: string
  /** Renders the overlay in its OPEN state. */
  render: () => ReactNode
  /** Optional wrapper (provider/host) the overlay needs. */
  wrapper?: ComponentType<{ children: ReactNode }>
}

/** No standalone-openable desktop overlays yet — see the module note above. */
export const OVERLAY_ENTRIES: OverlayEntry[] = []

export const WIRED_OVERLAY_SURFACES = new Set(OVERLAY_ENTRIES.map(o => o.surface))

export const overlayBySlug = (slug: string) =>
  OVERLAY_ENTRIES.find(o => o.slug === slug)

/**
 * Gallery coverage registry — the ENFORCED gate.
 *
 * `GALLERY_COVERAGE` maps EVERY generated `GallerySurface` (see
 * galleryCoverage.generated.ts) to how the gallery covers it. Because the object
 * is `satisfies Record<GallerySurface, Coverage>`, a surface with no entry is a
 * COMPILE error, and a stale key (deleted file) is also a compile error.
 *
 * Coverage kinds:
 *   - page(id)    — this surface IS a seeded gallery page (see ALL_PAGES).
 *   - story(id)   — covered by a kit story section (see stories/).
 *   - via(id)     — rendered inside another covered surface (its page/story).
 *   - allow(why)  — genuinely non-visual (provider/context/pure-logic/null
 *                   render) — no visual entry needed; the reason is reviewed.
 *   - pending(why)— tracked TODO: accounted for, not yet given a visual entry.
 *
 * The tsc gate guarantees every surface is at least `pending`; the parity test
 * (gen-gallery-coverage.mjs --check) lists pending surfaces so they stay visible.
 */
import type { GallerySurface } from './galleryCoverage.generated'

/**
 * A surface's KIND drives its REQUIRED STATE SET (enforced by the coverage gate,
 * `gen-gallery-coverage.mjs --check`). A surface whose declared `states` miss its
 * kind's required set fails the gate.
 *
 *   data-page / table → loaded + empty + error   (most bugs live in empty/error)
 *   form              → empty + filled + invalid
 *   overlay           → open
 *   static / flow / via / nonvisual / pending → none required
 *
 * Escape hatch: a genuinely stateless surface uses `static`; a non-visual one
 * uses `nonvisual`; both opt out of the required-state gate with a reason.
 */
export type SurfaceKind =
  | 'data-page'
  | 'table'
  | 'form'
  | 'overlay'
  | 'static'
  | 'flow'
  | 'via'
  | 'nonvisual'
  | 'pending'

/** Data-states rendered by swapping the mock cassette (see mockApi `MockMode`). */
export type GalleryState =
  | 'loaded'
  | 'empty'
  | 'error'
  | 'delayed'
  | 'open'
  | 'filled'
  | 'invalid'

export interface Coverage {
  kind: SurfaceKind
  /** Declared states for this surface (each a screenshot combo). */
  states?: readonly GalleryState[]
  /** Reason for via / nonvisual / pending / static opt-outs. */
  reason?: string
}

/** Required state set per kind — the gate. Empty = no required states. */
export const REQUIRED_STATES: Record<SurfaceKind, readonly GalleryState[]> = {
  'data-page': ['loaded', 'empty', 'error'],
  table: ['loaded', 'empty', 'error'],
  form: ['empty', 'filled', 'invalid'],
  overlay: ['open'],
  static: [],
  flow: [],
  via: [],
  nonvisual: [],
  pending: [],
}

// Keep this object total over GallerySurface (the tsc gate). The node gate
// (`gen-gallery-coverage.mjs --check`) additionally enforces REQUIRED_STATES.
export const GALLERY_COVERAGE = {
  "modules/host-mount/conversation-extension/components/ConversationMountsControl": { kind: 'static', reason: 'in-conversation host-mount control — open-state needs live conversation + host-mount context; verified via the e2e interaction suite' },
  "modules/host-mount/pages/HostMountPolicyPage": { kind: 'data-page', states: ['loaded', 'empty', 'error'] },
  "modules/host-mount/project-extension/components/ProjectMountsPanel": { kind: 'via', reason: 'rendered within the host-mount project extension panel' },
  "modules/host-mount/project-extension/extension": { kind: 'nonvisual', reason: 'project-extension registration' },
  "modules/memory/pages/MemoryCombinedPage": { kind: 'data-page', states: ['loaded', 'empty', 'error'] },
  "modules/remote-access/pages/RemoteAccessPage": { kind: 'data-page', states: ['loaded', 'empty', 'error'] },
  "modules/tunnel-auth/MagicLinkPage": { kind: 'flow', reason: 'magic-link auth flow' },
  "modules/tunnel-auth/PhoneAuthPage": { kind: 'flow', reason: 'phone auth flow' },
  "modules/updater/components/UpdateBanner": { kind: 'via', reason: 'slot banner in the app-layout' },
  "modules/updater/pages/AboutPage": { kind: 'data-page', states: ['loaded', 'empty', 'error'] },
  "modules/desktop-base/overrides/hardware-monitor": { kind: 'via', reason: 'desktop override for the hardware.monitor-button <Seam> — rendered inside the hardware settings header' },
  "modules/desktop-base/overrides/sidebar-header-spacer": { kind: 'via', reason: 'desktop override for the layout.sidebar-header-spacer <Seam> — the drag-enabled top spacer rendered inside the LeftSidebar' },
  // <<< scaffold-insert >>>
} satisfies Record<GallerySurface, Coverage>

// ── Rollup counts (used by COVERAGE.md + the coverage report) ────────────────
export function coverageSummary() {
  const counts = {} as Record<SurfaceKind, number>
  for (const v of Object.values(GALLERY_COVERAGE) as Coverage[]) {
    counts[v.kind] = (counts[v.kind] ?? 0) + 1
  }
  const total = Object.keys(GALLERY_COVERAGE).length
  const covered = total - (counts.pending ?? 0)
  return { total, covered, ...counts }
}

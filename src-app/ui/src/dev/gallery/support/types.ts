/**
 * The per-module gallery-seed contract + the shared surface-entry types.
 *
 * A module opts into the dev gallery by co-locating `src/modules/<X>/gallery.tsx`
 * that `export const gallery: ModuleGallery` — mirroring how it
 * `export default createModule(...)` in `module.tsx`. The gallery's runtime
 * registry (`support/registry.ts`) auto-discovers every such file via
 * `import.meta.glob`, exactly as `modules/loader.ts` discovers `module.tsx`. The
 * committed completeness gate (`scripts/gen-gallery-seed-registry.mjs`) fails a
 * build where a surface-bearing module has no `gallery.tsx`.
 *
 * Entry types moved here (verbatim) from the former central files so a per-module
 * `gallery.tsx` imports ONLY from `@/dev/gallery/support` — never a central file.
 */
import type { ComponentType, LazyExoticComponent } from 'react'
import type { Cassette } from '../mockApi'
import type { InteractionRecipe } from '../interactions'
import type { GalleryStory } from '../story'

export type { Cassette } from '../mockApi'
export type { InteractionRecipe } from '../interactions'
export type { GalleryStory } from '../story'

/** Overlay open-state entry (was `overlays.tsx::OverlayEntry`). */
export interface OverlayEntry {
  /** Gallery slug → `?surface=<slug>&state=open`; also the section testid. */
  slug: string
  /** Coverage surface id (the component file) — feeds the overlay-registry gate. */
  surface: string
  /** Human title for the frame. */
  title: string
  component: LazyExoticComponent<ComponentType>
  /** Seed + fire the store open action (runs on mount). Optional: prop-driven
   *  overlays render open via bound props (see `lazyBound`) with no store call. */
  open?: () => void | Promise<void>
  /** Interaction recipes driven after the overlay opens (focus an input, submit
   *  invalid, …). Driven via `?surface=<slug>&interact=<name>`. */
  interactions?: InteractionRecipe[]
}

/** Active-conversation deep-state entry (was `deepStates.tsx::DeepStateEntry`). */
export interface DeepStateEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. */
  slug: string
  /** Human title for the frame. */
  title: string
  /** Which conversation the ConversationPage is pinned to. */
  conversationId: string
  /** One-line note about what deep state this exercises. */
  note: string
  /** Seed the transient state through the real store (runs after mount). */
  setup?: () => void | Promise<void>
  /** Interaction recipes driven after mount (`?surface=<slug>&interact=<name>`). */
  interactions?: InteractionRecipe[]
}

/** Seeded real-component surface entry (was `seeded/helpers.tsx::SeededSurfaceEntry`). */
export interface SeededSurfaceEntry {
  /** Gallery slug → `?surface=<slug>`; also the section testid. Keep it UNIQUE. */
  slug: string
  /** Human title for the frame. */
  title: string
  /** One-line note about the seeded state this reaches. */
  note: string
  /** Route path the component is mounted under (for useParams/useNavigate). */
  path: string
  /** Concrete initial path (params filled). */
  initialPath: string
  /** The real component to render. */
  component: LazyExoticComponent<ComponentType>
  /** Seed the transient state through the real store (runs after mount). */
  setup?: () => void | Promise<void>
  /** Interaction recipes driven after the seeded surface mounts. */
  interactions?: InteractionRecipe[]
  /** Render at natural height instead of the fixed 720px overflow-hidden frame. */
  fullHeight?: boolean
}

/**
 * A module's gallery seed — everything the dev gallery needs to render THIS
 * module's surfaces offline. Every field is optional; a module fully covered by
 * the shared crawl cassette sets `crawlOnly: true` (a conscious ownership marker,
 * not an omission).
 */
export interface ModuleGallery {
  /** Mock-API seed BEYOND the shared crawl base: detail-route/query resolvers,
   *  mutations, richer list data. Typed vs the generated api-client. */
  cassette?: Cassette
  /** Overlay open-states this module exposes. */
  overlays?: OverlayEntry[]
  /** Active-conversation deep-states (chat only, by construction). */
  deepStates?: DeepStateEntry[]
  /** Real-component + store-seed surfaces. */
  seeded?: SeededSurfaceEntry[]
  /** Kit-component stories (rare per-module; design-system stories stay central). */
  stories?: GalleryStory[]
  /** This module's pages render fully from the shared crawl base — no bespoke
   *  seed needed. Present so a crawl-covered module still declares ownership and
   *  satisfies the completeness gate. */
  crawlOnly?: true
}

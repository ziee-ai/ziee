/**
 * The ONLY import surface a per-module `src/modules/<X>/gallery.tsx` needs:
 * the ziee-bound entry types + the authoring helpers (from `@ziee/gallery`).
 */
export type {
  Cassette,
  CassetteEntry,
  DeepStateEntry,
  GalleryStory,
  InteractionRecipe,
  ModuleGallery,
  OverlayEntry,
  SeededSurfaceEntry,
} from './types'

export {
  holdForever,
  holdPatch,
  whenTrue,
  lazyBound,
  lazyCompose,
  lazyNamed,
  lazyProps,
} from '@ziee/gallery'

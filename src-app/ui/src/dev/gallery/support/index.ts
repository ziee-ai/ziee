/**
 * The ONLY import surface a per-module `src/modules/<X>/gallery.tsx` needs:
 * the entry types + the authoring helpers. Keeps per-module seed decoupled from
 * the gallery's central files.
 */
export type {
  Cassette,
  DeepStateEntry,
  GalleryStory,
  InteractionRecipe,
  ModuleGallery,
  OverlayEntry,
  SeededSurfaceEntry,
} from './types'

export { holdForever, holdPatch, whenTrue } from './hold'
export { lazyBound, lazyCompose, lazyNamed, lazyProps } from './lazy'

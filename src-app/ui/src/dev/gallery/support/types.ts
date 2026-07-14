/**
 * ziee's BINDING of the generic `@ziee/gallery` authoring contract to this app's
 * generated api-client. `Cassette` / `ModuleGallery` are generic in the package
 * (over the app's `ApiEndpointResponses` map); binding them here to
 * `@/api-client/types` is what preserves the compile-time cassette-shape check:
 * every per-module `gallery.tsx` writes `const gallery: ModuleGallery = { cassette }`
 * and its cassette is validated against the generated response types at `tsc` time.
 *
 * The 36 per-module `gallery.tsx` files import ONLY from `@/dev/gallery/support`,
 * so this file (+ the barrel) is the single seam between ziee content and the
 * extracted framework.
 */
import type { ApiEndpointResponses } from '@/api-client/types'
import type {
  Cassette as GCassette,
  CassetteEntry as GCassetteEntry,
  ModuleGallery as GModuleGallery,
} from '@ziee/gallery'

/** ziee-bound cassette: typed against the generated api-client response map. */
export type Cassette = GCassette<ApiEndpointResponses>
export type CassetteEntry<K extends keyof ApiEndpointResponses> =
  GCassetteEntry<ApiEndpointResponses[K]>
/** ziee-bound module-gallery contract (`cassette` checked vs the api-client). */
export type ModuleGallery = GModuleGallery<ApiEndpointResponses>

export type {
  OverlayEntry,
  DeepStateEntry,
  SeededSurfaceEntry,
  GalleryStory,
  InteractionRecipe,
} from '@ziee/gallery'

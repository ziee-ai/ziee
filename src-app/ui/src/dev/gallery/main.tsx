/**
 * Standalone (backend-free) entry for the component gallery.
 *
 * Served by the Vite dev server at `/gallery.html`. Unlike the in-app
 * `/dev/gallery` route, this does NOT boot the module system, auth, or any
 * backend call — `mountGallery` installs the mock-API cassette, seeds an
 * authenticated admin, loads every module so its stores resolve, and renders the
 * real ThemeProvider + gallery. A deterministic, fully static canvas for the
 * Playwright layout + screenshot layers.
 *
 * All ziee-specific dependency injection lives in `buildGalleryConfig()`; this
 * file is the thin ~5-line boot the extraction leaves app-side.
 */
import { mountGallery } from '@ziee/gallery'
import { buildGalleryConfig } from './galleryConfig'
import '@/index.css'

mountGallery(buildGalleryConfig())

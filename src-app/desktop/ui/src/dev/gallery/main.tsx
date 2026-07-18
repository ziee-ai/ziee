/**
 * Standalone (backend-free) entry for the DESKTOP component gallery.
 *
 * Served by the Vite dev server at `/gallery.html` (Vite root is `src/`). Unlike
 * the in-app route, this does NOT boot a live backend — `mountGallery` installs
 * the mock-API cassette, seeds an authenticated admin, loads every module (core +
 * desktop) so its stores resolve, and renders the real ThemeProvider + gallery.
 * A deterministic, fully static canvas for the Playwright layout + screenshot
 * layers.
 *
 * All ziee-desktop-specific dependency injection lives in `buildGalleryConfig()`;
 * this file is the thin ~4-line boot the extraction leaves app-side, identical in
 * shape to the web workspace's `main.tsx`.
 */
import { mountGallery } from '@ziee/gallery'
import { buildGalleryConfig } from './galleryConfig'
import '@/index.css'

mountGallery(buildGalleryConfig())

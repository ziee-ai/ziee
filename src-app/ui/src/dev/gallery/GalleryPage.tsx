/**
 * ziee's `GalleryPage` entry for the IN-APP `/dev/gallery` route (rendered inside
 * the live, booted app shell — the actual ThemeProvider/tokens/accent apply).
 *
 * The standalone `/gallery.html` entry calls `mountGallery(cfg)` (which sets the
 * config, installs the mock API, seeds auth, and renders). This in-app route does
 * NOT boot the standalone entry, so we set the framework config + assemble the
 * discovered surfaces here (WITHOUT installing the mock — the in-app gallery
 * renders against the running app), then re-export the package `GalleryPage`.
 */
import {
  GalleryPage as FrameworkGalleryPage,
  initSurfaces,
  setGalleryConfig,
} from '@ziee/gallery'
import { buildGalleryConfig } from './galleryConfig'

const cfg = buildGalleryConfig()
setGalleryConfig(cfg)
initSurfaces(cfg.discoverGalleries())

export const GalleryPage = FrameworkGalleryPage
export default FrameworkGalleryPage

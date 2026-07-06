import { createModule } from '@/core'
import { BlankLayout } from '@/modules/layouts/blank'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

/**
 * Dev-only module that mounts the component gallery at `/dev/gallery` inside the
 * real app shell (so the actual `ThemeProvider`, tokens, and accent apply). The
 * route is registered ONLY in dev builds (`import.meta.env.DEV`); production
 * bundles get an empty route list, so the gallery never ships.
 *
 * The canonical, backend-free surface used by the Playwright visual layers is
 * the standalone `/gallery.html` entry; this route is for manual review in a
 * running, set-up instance.
 */
const GalleryPage = lazyWithPreload(() =>
  import('@/dev/gallery/GalleryPage').then(m => ({ default: m.GalleryPage })),
)

export default createModule({
  metadata: {
    name: 'dev-gallery',
    version: '1.0.0',
    description: 'Dev-only component gallery (visual-testing canvas)',
  },
  routes: import.meta.env.DEV
    ? [
        {
          path: '/dev/gallery',
          element: GalleryPage,
          requiresAuth: false,
          layout: BlankLayout,
        },
      ]
    : [],
})

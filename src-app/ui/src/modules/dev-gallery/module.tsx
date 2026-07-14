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
export default createModule({
  metadata: {
    name: 'dev-gallery',
    version: '1.0.0',
    description: 'Dev-only component gallery (visual-testing canvas)',
  },
  // The `import('@/dev/gallery/GalleryPage')` reference lives INSIDE the
  // `import.meta.env.DEV` branch so that in a prod build the whole array literal
  // is dead code — Rollup drops the reference and never emits a GalleryPage chunk
  // (a top-level `const GalleryPage = lazyWithPreload(...)` would keep the dynamic
  // import reachable and ship the gallery + its mock-cassette data as a lazy
  // chunk). Enforced by scripts/check-gallery-prod-exclusion.mjs.
  routes: import.meta.env.DEV
    ? [
        {
          path: '/dev/gallery',
          element: lazyWithPreload(() =>
            import('@/dev/gallery/GalleryPage').then(m => ({
              default: m.GalleryPage,
            })),
          ),
          requiresAuth: false,
          layout: BlankLayout,
        },
      ]
    : [],
})

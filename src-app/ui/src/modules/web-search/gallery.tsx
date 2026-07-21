/**
 * Dev-gallery seed for the `web-search` module — the settings sections in their
 * stuck-loading state. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyCompose } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── web_search sections (rendered direct): stuck loading (both arms). ────────
    {
      slug: 'seeded-web-search-loading',
      title: 'Web Search settings — loading',
      note: 'loading && !settings / loading && providers.length===0 → both section loaders',
      path: '/',
      initialPath: '/',
      component: lazyCompose([
        {
          loader: () => import('@/modules/web-search/components/WebSearchGlobalSection'),
          name: 'WebSearchGlobalSection',
        },
        {
          loader: () => import('@/modules/web-search/components/WebSearchProvidersSection'),
          name: 'WebSearchProvidersSection',
        },
      ]),
      setup: async () => {
        const { WebSearchAdmin } = await import(
          '@/modules/web-search/stores/webSearchAdmin'
        )
        await holdPatch(() =>
          WebSearchAdmin.__setState({
            loading: true,
            settings: null,
            providers: [],
          } as any),
        )
      },
    },
  ],
}

/**
 * Dev-gallery seed for the `literature` module — the connectors settings section
 * in its stuck-loading state + the literature tool-result card with an empty
 * records array. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, lazyProps } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── lit_search connectors section: stuck loading (loading && no connectors). ─
    {
      slug: 'seeded-literature-loading',
      title: 'Literature settings — loading',
      note: 'loading && connectors.length===0 → the connectors-section loader',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/literature/components/settings/LitSearchConnectorsSection'),
        'LitSearchConnectorsSection',
      ),
      setup: async () => {
        const { useLitSearchAdminStore } = await import(
          '@/modules/literature/stores/litSearchAdmin'
        )
        await holdPatch(() =>
          useLitSearchAdminStore.setState({
            loading: true,
            settings: null,
            connectors: [],
          } as any),
        )
      },
    },
    // ── LiteratureToolResultCard: a literature_search result whose records array
    //    is empty → the "No records returned" empty text. Pure prop-driven
    //    content renderer (no store). ───────────────────────────────────────────
    {
      slug: 'seeded-s4-lit-tool-result-empty',
      title: 'Literature tool result — empty',
      note: 'sc.records.length===0 → the "No records returned" empty text',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/literature/components/LiteratureToolResultCard'),
        'LiteratureToolResultCard',
        {
          isUser: false,
          content: {
            content_type: 'tool_result',
            content: {
              name: 'literature_search',
              tool_use_id: 'lit-s4',
              structured_content: {
                query: 'crispr base editing safety',
                records: [],
                identified: { europepmc: 0, crossref: 0 },
                after_dedup: 0,
                degraded_sources: [],
                completeness: null,
              },
            },
          },
        },
      ),
    },
  ],
}

/**
 * Dev-gallery seed for the `llm-repository` module — the model-repository
 * drawer. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed } from '@/dev/gallery/support'
import { Stores } from '@/core/stores'

export const gallery: ModuleGallery = {
  overlays: [
    {
      slug: 'overlay-llm-repository-drawer',
      surface: 'modules/llm-repository/components/LlmRepositoryDrawer',
      title: 'LLM Repository (drawer)',
      component: lazyNamed(
        () => import('@/modules/llm-repository/components/LlmRepositoryDrawer'),
        'LlmRepositoryDrawer',
      ),
      open: () => Stores.LlmRepositoryDrawer.openDrawer(),
    },
  ],
}

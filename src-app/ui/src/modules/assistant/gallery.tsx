/**
 * Dev-gallery seed for the `assistant` module — the create/edit assistant form
 * drawer rendered OPEN. Auto-discovered by the gallery's runtime registry
 * (`@/dev/gallery/support`); never imported by `module.tsx`, so it is dev-only
 * and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyNamed } from '@/dev/gallery/support'
import { Stores } from '@/core/stores'

export const gallery: ModuleGallery = {
  overlays: [
    {
      slug: 'overlay-assistant-form-drawer',
      surface: 'modules/assistant/components/AssistantFormDrawer',
      title: 'Create Assistant (drawer)',
      component: lazyNamed(
        () => import('@/modules/assistant/components/AssistantFormDrawer'),
        'AssistantFormDrawer',
      ),
      open: () => Stores.AssistantDrawer.openAssistantDrawer(),
    },
  ],
}

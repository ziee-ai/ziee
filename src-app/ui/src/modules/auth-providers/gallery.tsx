/**
 * Dev-gallery seed for the `auth-providers` module — the auth-provider edit
 * drawer rendered OPEN via bound props. Auto-discovered by the gallery's runtime
 * registry (`@/dev/gallery/support`); never imported by `module.tsx`, so it is
 * dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { lazyBound } from '@/dev/gallery/support'

const noop = () => {}

export const gallery: ModuleGallery = {
  overlays: [
    {
      slug: 'overlay-auth-provider-edit-drawer',
      surface: 'modules/auth-providers/components/AuthProviderEditDrawer',
      title: 'Edit auth provider (drawer)',
      component: lazyBound(
        () => import('@/modules/auth-providers/components/AuthProviderEditDrawer'),
        'AuthProviderEditDrawer',
        { open: true, onClose: noop },
      ),
    },
  ],
}

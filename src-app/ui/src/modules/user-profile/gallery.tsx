/**
 * Dev-gallery seed for the `user-profile` module — the UserProfileWidget in its
 * auth-still-resolving skeleton state. Auto-discovered by the gallery's runtime
 * registry (`@/dev/gallery/support`); never imported by `module.tsx`, so it is
 * dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdForever, lazyNamed } from '@/dev/gallery/support'

export const gallery: ModuleGallery = {
  seeded: [
    // ── UserProfileWidget: auth still resolving (!user && (isInitializing||isLoading)). ─
    {
      slug: 'seeded-s5-user-profile-loading',
      title: 'User profile widget — loading',
      note: '!user && (isInitializing || isLoading) → the skeleton row (UserProfileWidget:86)',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/user-profile/UserProfileWidget'),
        'UserProfileWidget',
      ),
      setup: async () => {
        const { Auth } = await import('@/modules/auth/Auth.store')
        // holdForever (not holdPatch): the widget can mount after a fixed hold
        // window ends under the full pass, so assert on a permanent interval.
        holdForever(() =>
          Auth.store.setState({
            user: null,
            isInitializing: true,
            isLoading: false,
          } as any),
        )
      },
    },
  ],
}

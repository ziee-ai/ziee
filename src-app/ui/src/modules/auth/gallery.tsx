/**
 * Dev-gallery seed for the `auth` module — the OAuth provider-button states,
 * login/register submit-error arms, the link-account form, and the AuthGuard
 * bootstrap spinner. Owns the shared `authCassette`.
 * Auto-discovered by the gallery's runtime registry (`@/dev/gallery/support`);
 * never imported by `module.tsx`, so it is dev-only and tree-shaken from prod.
 */
import type { ModuleGallery } from '@/dev/gallery/support'
import { holdPatch, lazyNamed, lazyProps } from '@/dev/gallery/support'
import { authCassette } from '@/dev/gallery/fixtures/auth'

export const gallery: ModuleGallery = {
  // `authCassette` already seeds `Auth.getSessionSettings` (for the
  // `/settings/sessions` admin page) + `Auth.me` etc. — no inline override needed.
  cassette: authCassette,
  seeded: [
    {
      slug: 'seeded-provider-buttons-loading',
      title: 'OAuth provider buttons — loading',
      note: 'isLoading || !hasLoaded → the "Loading sign-in options" spinner',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/auth/ProviderButtons'),
        'ProviderButtons',
      ),
      setup: async () => {
        const { AuthProviders } = await import('@/modules/auth/authProviders')
        await holdPatch(() =>
          AuthProviders.__setState({ isLoading: true, hasLoaded: false } as any),
        )
      },
    },
    {
      slug: 'seeded-provider-buttons-error',
      title: 'OAuth provider buttons — error',
      note: 'error (loaded) → "Unable to load sign-in options" alert',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/auth/ProviderButtons'),
        'ProviderButtons',
      ),
      setup: async () => {
        const { AuthProviders } = await import('@/modules/auth/authProviders')
        await holdPatch(() =>
          AuthProviders.__setState({
            isLoading: false,
            hasLoaded: true,
            error: 'Unable to reach the sign-in service.',
            providers: [],
          } as any),
        )
      },
    },
    {
      slug: 'seeded-provider-buttons-empty',
      title: 'OAuth provider buttons — none configured',
      note: '!providers.length → renders nothing (no external sign-in)',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/auth/ProviderButtons'),
        'ProviderButtons',
      ),
      setup: async () => {
        const { AuthProviders } = await import('@/modules/auth/authProviders')
        await holdPatch(() =>
          AuthProviders.__setState({
            isLoading: false,
            hasLoaded: true,
            error: null,
            providers: [],
          } as any),
        )
      },
    },
    {
      slug: 'seeded-login-error',
      title: 'Login form — error',
      note: 'Stores.Auth.error → the login error alert',
      path: '/',
      initialPath: '/',
      component: lazyNamed(() => import('@/modules/auth/LoginForm'), 'LoginForm'),
      setup: async () => {
        const { Auth } = await import('@/modules/auth/Auth.store')
        await holdPatch(() =>
          Auth.store.setState({ error: 'Invalid email or password.' } as any),
        )
      },
    },
    {
      slug: 'seeded-register-error',
      title: 'Register form — error',
      note: 'Stores.Auth.error → the register error alert',
      path: '/',
      initialPath: '/',
      component: lazyNamed(
        () => import('@/modules/auth/RegisterForm'),
        'RegisterForm',
      ),
      setup: async () => {
        const { Auth } = await import('@/modules/auth/Auth.store')
        await holdPatch(() =>
          Auth.store.setState({ error: 'That email is already registered.' } as any),
        )
      },
    },
    {
      slug: 'auth-link-account',
      title: 'Link account — password confirm form',
      note: 'the page shows a missing-token error banner without ?link_token=; mount with a token so the real form renders (the banner is a separate state, not "empty").',
      path: '/auth/link-account',
      initialPath: '/auth/link-account?link_token=gallery-demo-link-token',
      component: lazyNamed(
        () => import('@/modules/auth/LinkAccountPage'),
        'LinkAccountPage',
      ),
    },
    {
      slug: 'seeded-s5-auth-initializing',
      title: 'Auth guard — bootstrap loading',
      note: 'multiUser && (isInitializing || needsSetup===null) → fullscreen spinner (AuthGuard:47)',
      path: '/',
      initialPath: '/',
      component: lazyProps(
        () => import('@/modules/auth/AuthGuard'),
        'AuthGuard',
        { children: null },
      ),
      setup: async () => {
        const { Auth } = await import('@/modules/auth/Auth.store')
        const { App } = await import('@/modules/app/stores/app')
        const { AppMode } = await import('@/modules/app/AppMode.store')
        await holdPatch(() => {
          AppMode.__setState({ multiUserMode: true } as any)
          App.__setState({ needsSetup: null })
          Auth.store.setState({
            isInitializing: true,
            isAuthenticated: false,
          } as any)
        })
      },
    },
  ],
}

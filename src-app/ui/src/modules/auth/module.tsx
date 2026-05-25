import { createModule } from '@/core'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { useAuthProvidersStore } from '@/modules/auth/AuthProviders.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const AuthPage = lazyWithPreload(() =>
  import('./AuthPage').then(m => ({ default: m.AuthPage })),
)
const AuthCallbackPage = lazyWithPreload(() =>
  import('./AuthCallbackPage').then(m => ({ default: m.AuthCallbackPage })),
)
const LinkAccountPage = lazyWithPreload(() =>
  import('./LinkAccountPage').then(m => ({ default: m.LinkAccountPage })),
)

export default createModule({
  metadata: {
    name: 'auth',
    version: '1.0.0',
    description: 'Authentication module with login, registration, and social sign-in',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/login',
      element: AuthPage,
    },
    {
      path: '/auth/callback',
      element: AuthCallbackPage,
    },
    {
      path: '/auth/link-account',
      element: LinkAccountPage,
    },
  ],
  stores: [
    {
      name: 'Auth',
      store: useAuthStore,
    },
    {
      name: 'AuthProviders',
      store: useAuthProvidersStore,
    },
  ],
  initialize: () => {
    console.log('Auth module initialized')
  },
})

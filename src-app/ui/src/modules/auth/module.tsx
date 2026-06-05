import { createModule } from '@/core'
import '@/modules/auth/sync' // registerSync('session') side-effect
import { useAuthStore } from '@/modules/auth/Auth.store'
import { useAuthProvidersStore } from '@/modules/auth/AuthProviders.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { AuthGuard } from './AuthGuard'

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
      path: '/auth',
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
  // Fill the router-owned `routeGuards` slot so the router gates protected
  // routes without importing anything from auth (inverts router→auth).
  slots: {
    routeGuards: [{ id: 'auth-guard', component: AuthGuard }],
  },
  initialize: () => {
    console.log('Auth module initialized')
  },
})

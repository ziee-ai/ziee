import { TimerReset } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useAuthStore } from '@/modules/auth/Auth.store'
import { useAuthProvidersStore } from '@/modules/auth/authProviders'
import { useSessionSettingsStore } from '@/modules/auth/sessionSettings'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
// Import via the `@/` alias (NOT a relative './AuthGuard') so the desktop
// build's vite-plugin-local-override can redirect this to the desktop
// AuthGuard override (the plugin only rewrites `@/`-prefixed specifiers;
// a relative import would always bind core's AuthGuard, even on desktop —
// bypassing the deliberate desktop divergence). Resolves to core's
// AuthGuard.tsx on the web build.
import { AuthGuard } from '@/modules/auth/AuthGuard'

const AuthPage = lazyWithPreload(() =>
  import('./AuthPage').then(m => ({ default: m.AuthPage })),
)
const AuthCallbackPage = lazyWithPreload(() =>
  import('./AuthCallbackPage').then(m => ({ default: m.AuthCallbackPage })),
)
const LinkAccountPage = lazyWithPreload(() =>
  import('./LinkAccountPage').then(m => ({ default: m.LinkAccountPage })),
)
const SessionSettingsPage = lazyWithPreload(() =>
  import('./SessionSettingsPage').then(m => ({
    default: m.SessionSettingsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'auth',
    version: '1.0.0',
    description:
      'Authentication module with login, registration, and social sign-in',
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
    {
      path: '/settings/sessions',
      element: SessionSettingsPage,
      requiresAuth: true,
      permission: Permissions.SessionSettingsRead,
      layout: SettingsLayoutDef,
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
    {
      name: 'SessionSettings',
      store: useSessionSettingsStore,
    },
  ],
  // Fill the router-owned `routeGuards` slot so the router gates protected
  // routes without importing anything from auth (inverts router→auth).
  slots: {
    routeGuards: [{ id: 'auth-guard', component: AuthGuard }],
    settingsAdminPages: [
      {
        id: 'sessions',
        icon: <TimerReset />,
        label: 'Sessions',
        path: 'sessions',
        order: 29,
        permission: Permissions.SessionSettingsRead,
      },
    ],
  },
  initialize: () => {
    console.log('Auth module initialized')
  },
})

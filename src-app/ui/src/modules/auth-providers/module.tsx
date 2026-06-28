import { createModule } from '@/core'
import { Lock } from 'lucide-react'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { Permissions } from '@/api-client/types'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useAuthProvidersAdminStore } from './stores/AuthProvidersAdmin.store'
import './types' // CRITICAL: store type declaration merging

const AuthProvidersSettingsPage = lazyWithPreload(() =>
  import('./AuthProvidersSettingsPage').then(m => ({
    default: m.AuthProvidersSettingsPage,
  })),
)

// Single source of truth: slot filter (menu) + route gate (deep-link
// 403) share this same expression.
const AUTH_PROVIDERS_READ = Permissions.AuthProvidersRead

export default createModule({
  metadata: {
    name: 'auth-providers',
    version: '1.0.0',
    description: 'Admin: configure third-party auth providers (Google, Microsoft, Apple, generic OIDC)',
  },
  dependencies: ['router', 'auth'],
  routes: [
    {
      path: '/settings/auth-providers',
      element: AuthProvidersSettingsPage,
      requiresAuth: true,
      permission: AUTH_PROVIDERS_READ,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'AuthProvidersAdmin',
      store: useAuthProvidersAdminStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'auth-providers',
        icon: <Lock />,
        label: 'Auth Providers',
        path: 'auth-providers',
        order: 22,
        permission: AUTH_PROVIDERS_READ,
      },
    ],
  },
})

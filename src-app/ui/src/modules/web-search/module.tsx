import { Globe, KeyRound } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import './types' // CRITICAL: enable store type declaration merging

const WebSearchSettingsPage = lazyWithPreload(() =>
  import('./components/WebSearchSettingsPage').then(m => ({
    default: m.WebSearchSettingsPage,
  })),
)

const WebSearchUserKeysPage = lazyWithPreload(() =>
  import('./components/WebSearchUserKeysPage').then(m => ({
    default: m.WebSearchUserKeysPage,
  })),
)

export default createModule({
  metadata: {
    name: 'web-search',
    version: '1.0.0',
    description: 'Web search + page fetch admin settings (provider chain + keys)',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated,
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/web-search',
      element: WebSearchSettingsPage,
      requiresAuth: true,
      permission: Permissions.WebSearchAdminRead,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/web-search-keys',
      element: WebSearchUserKeysPage,
      requiresAuth: true,
      permission: Permissions.WebSearchUse,
      layout: SettingsLayoutDef,
    },
  ],
  // WebSearchAdmin is a settings-page store (only /settings/web-search reads it).
  // It's a registerLazyStore proxy that self-registers when that page imports it,
  // so listing it here — which loaded webSearchAdmin.js on EVERY route at module
  // registration (on login) — is intentionally omitted.
  stores: [],
  slots: {
    settingsAdminPages: [
      {
        id: 'web-search',
        icon: <Globe />,
        label: 'Web Search',
        path: 'web-search',
        order: 27,
        permission: Permissions.WebSearchAdminRead,
      },
    ],
    settingsUserPages: [
      {
        id: 'web-search-keys',
        icon: <KeyRound />,
        label: 'Web Search Keys',
        path: 'web-search-keys',
        order: 16,
        permission: Permissions.WebSearchUse,
      },
    ],
  },
})

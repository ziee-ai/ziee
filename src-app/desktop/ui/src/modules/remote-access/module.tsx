/**
 * Desktop Remote Access module.
 *
 * Owns the settings page at `/settings/remote-access` that configures
 * the embedded ngrok tunnel. Registered ONLY in the desktop bundle
 * (this directory) so the web bundle phones load over the tunnel
 * doesn't include the code, can't navigate to the page, and can't
 * disable the tunnel they're using.
 *
 * Backend control surface: `/api/remote-access/*` HTTP routes (see
 * `server/src/modules/remote_access/`). The localhost-Host middleware
 * on those routes is the defense-in-depth layer for the same goal.
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { lazy } from 'react'
import { Globe } from 'lucide-react'
import { SettingsLayoutDef } from '@ziee/ui-core/modules/settings/SettingsLayout'
import { Permissions } from '@/api-client/types'

import { useRemoteAccessStore } from '@ziee/desktop/modules/remote-access/stores/RemoteAccess.store'

const RemoteAccessPage = lazy(() =>
  import('./pages/RemoteAccessPage').then((m) => ({
    default: m.RemoteAccessPage,
  })),
)

const remoteAccessModule: AppModule = createModule({
  metadata: {
    name: 'remote-access-desktop',
    version: '1.0.0',
    description:
      'Desktop-only Remote Access settings: ngrok tunnel control + magic-link QR.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/remote-access',
      element: RemoteAccessPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
      permission: Permissions.RemoteAccessRead,
    },
  ],
  stores: [
    {
      name: 'RemoteAccess',
      store: useRemoteAccessStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'remote-access',
        icon: <Globe />,
        label: 'Remote Access',
        path: 'remote-access',
        // After the existing admin entries (memory-admin = 60-ish)
        // but before tear-down / about pages.
        order: 90,
        // Hide from non-admins (matches the pattern of every other
        // settingsAdminPages slot entry — without this, the sidebar
        // advertises a page the user gets 403 on every API call from).
        permission: Permissions.RemoteAccessRead,
      },
    ],
  },
})

export default remoteAccessModule

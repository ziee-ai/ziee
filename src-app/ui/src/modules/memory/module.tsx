import { createModule } from '@ziee/framework'
import { Brain } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types'

// The user page renders sections for each memory mechanism. A user
// with EITHER MemoryRead or CoreMemoryRead should reach the page —
// the missing section is hidden inside, but the page itself is
// accessible. Mirrors the `anyOf` pattern used by code-sandbox.
const MEMORY_USER_READ_PERM = {
  anyOf: [Permissions.MemoryRead, Permissions.CoreMemoryRead],
}

const MemorySettingsPage = lazyWithPreload(() =>
  import('./pages/MemorySettingsPage').then(m => ({
    default: m.MemorySettingsPage,
  })),
)
const MemoryAdminPage = lazyWithPreload(() =>
  import('./pages/MemoryAdminPage').then(m => ({ default: m.MemoryAdminPage })),
)

export default createModule({
  metadata: {
    name: 'memory',
    version: '1.0.0',
    description: 'Per-user persistent memory: settings + admin.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/memory',
      element: MemorySettingsPage,
      requiresAuth: true,
      permission: MEMORY_USER_READ_PERM,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/memory-admin',
      element: MemoryAdminPage,
      requiresAuth: true,
      permission: Permissions.MemoryAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'memory',
        icon: <Brain />,
        label: 'Memory',
        path: 'memory',
        order: 30,
        permission: MEMORY_USER_READ_PERM,
      },
    ],
    settingsAdminPages: [
      {
        id: 'memory-admin',
        icon: <Brain />,
        label: 'Memory',
        // Single-segment path (`memory-admin`, not `admin/memory`) so
        // SettingsPage's currentSection regex
        // `/\/settings\/([^/]+)/` captures the full key and the menu
        // auto-highlights when the URL is `/settings/memory-admin`.
        // Also keeps it distinct from the user-side `memory` slot
        // (SettingsPage matches on id, so the two-segment trick from
        // the previous incarnation is no longer needed).
        path: 'memory-admin',
        order: 60,
        permission: Permissions.MemoryAdminRead,
      },
    ],
  },
})

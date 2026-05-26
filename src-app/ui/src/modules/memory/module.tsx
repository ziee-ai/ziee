import { createModule } from '@/core'
import { BulbOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { Permissions } from '@/api-client/types'
import { useMemoriesStore } from './stores/Memories.store'
import { useMemorySettingsStore } from './stores/MemorySettings.store'
import { useMemoryAdminStore } from './stores/MemoryAdmin.store'
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
  import('./pages/MemorySettingsPage').then((m) => ({ default: m.MemorySettingsPage })),
)
const MemoryAdminPage = lazyWithPreload(() =>
  import('./pages/MemoryAdminPage').then((m) => ({ default: m.MemoryAdminPage })),
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
      path: '/settings/admin/memory',
      element: MemoryAdminPage,
      requiresAuth: true,
      permission: Permissions.MemoryAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    { name: 'Memories', store: useMemoriesStore },
    { name: 'MemorySettings', store: useMemorySettingsStore },
    { name: 'MemoryAdmin', store: useMemoryAdminStore },
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'memory',
        icon: <BulbOutlined />,
        label: 'Memory',
        path: 'memory',
        order: 30,
        permission: MEMORY_USER_READ_PERM,
      },
    ],
    settingsAdminPages: [
      {
        id: 'memory',
        icon: <BulbOutlined />,
        label: 'Memory',
        path: 'memory',
        order: 60,
        permission: Permissions.MemoryAdminRead,
      },
    ],
  },
  initialize: () => {
    // Pre-fetch admin settings so MemoryStatusPill renders correctly
    // on first paint (audit R7-#10). Without this, the chat composer
    // briefly shows the pill before discovering memory is admin-
    // disabled. Non-admin 403s are intentionally swallowed: pill
    // visibility falls back to "shown" (settings undefined ≠ disabled).
    import('@/core/stores').then(({ Stores }) => {
      Stores.MemoryAdmin.load().catch(() => {})
    })
  },
})

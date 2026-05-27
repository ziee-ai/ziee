import { createModule } from '@/core'
import { BulbOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { Permissions } from '@/api-client/types'
import { useMemoriesStore } from './stores/Memories.store'
import { useMemorySettingsStore } from './stores/MemorySettings.store'
import { useMemoryAdminStore } from './stores/MemoryAdmin.store'
import { useMemoryAuditStore } from './stores/MemoryAudit.store'
import { useCoreMemoryBlocksStore } from './stores/CoreMemoryBlocks.store'
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
    { name: 'MemoryAudit', store: useMemoryAuditStore },
    { name: 'CoreMemoryBlocks', store: useCoreMemoryBlocksStore },
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
        // Two-segment path: clicking the sidebar item navigates to
        // `/settings/admin/memory` (matches the registered route).
        // Critically, this also keeps it from colliding with the
        // user-side settingsUserPages slot's `path: 'memory'` — the
        // SettingsPage's `forbiddenSettingsItems.find(path === ...)`
        // is keyed on path-equality, so equal paths cause the
        // user-side `/settings/memory` URL to spuriously match the
        // admin entry and 403 even when the user has the user-side
        // read perm.
        path: 'admin/memory',
        order: 60,
        permission: Permissions.MemoryAdminRead,
      },
    ],
  },
})

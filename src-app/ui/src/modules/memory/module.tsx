import { createModule } from '@/core'
import { BulbOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { Permissions } from '@/api-client/types'
import { useMemoriesStore } from './stores/Memories.store'
import { useMemorySettingsStore } from './stores/MemorySettings.store'
import { useMemoryAdminStore } from './stores/MemoryAdmin.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types'

const MemoriesPage = lazyWithPreload(() =>
  import('./pages/MemoriesPage').then((m) => ({ default: m.MemoriesPage })),
)
const MemorySettingsPage = lazyWithPreload(() =>
  import('./pages/MemorySettingsPage').then((m) => ({ default: m.MemorySettingsPage })),
)
const MemoryAdminPage = lazyWithPreload(() =>
  import('./pages/MemoryAdminPage').then((m) => ({ default: m.MemoryAdminPage })),
)
const CoreMemoryPage = lazyWithPreload(() =>
  import('./pages/CoreMemoryPage').then((m) => ({ default: m.CoreMemoryPage })),
)

export default createModule({
  metadata: {
    name: 'memory',
    version: '1.0.0',
    description: 'Per-user persistent memory: list, settings, admin.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/memories',
      element: MemoriesPage,
      requiresAuth: true,
      permission: Permissions.MemoryRead,
      layout: AppLayoutDef,
    },
    {
      path: '/settings/memory',
      element: MemorySettingsPage,
      requiresAuth: true,
      permission: Permissions.MemoryRead,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/admin/memory',
      element: MemoryAdminPage,
      requiresAuth: true,
      permission: Permissions.MemoryAdminRead,
      layout: SettingsLayoutDef,
    },
    {
      path: '/memories/core-memory',
      element: CoreMemoryPage,
      requiresAuth: true,
      permission: Permissions.CoreMemoryRead,
      layout: AppLayoutDef,
    },
  ],
  stores: [
    { name: 'Memories', store: useMemoriesStore },
    { name: 'MemorySettings', store: useMemorySettingsStore },
    { name: 'MemoryAdmin', store: useMemoryAdminStore },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'memories',
        icon: <BulbOutlined />,
        label: 'Memories',
        path: '/memories',
        order: 30,
        permission: Permissions.MemoryRead,
      },
    ],
    settingsUserPages: [
      {
        id: 'memory',
        icon: <BulbOutlined />,
        label: 'Memory',
        path: 'memory',
        order: 30,
        permission: Permissions.MemoryRead,
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
    console.log('Memory module initialized')
  },
})

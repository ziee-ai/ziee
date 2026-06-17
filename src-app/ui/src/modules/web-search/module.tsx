import { GlobalOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useWebSearchAdminStore } from './stores/WebSearchAdmin.store'
import './types' // CRITICAL: enable store type declaration merging

const WebSearchSettingsPage = lazyWithPreload(() =>
  import('./components/WebSearchSettingsPage').then(m => ({
    default: m.WebSearchSettingsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'web-search',
    version: '1.0.0',
    description: 'Web search + page fetch admin settings (provider chain + keys)',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/web-search',
      element: WebSearchSettingsPage,
      requiresAuth: true,
      permission: Permissions.WebSearchAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'WebSearchAdmin',
      store: useWebSearchAdminStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'web-search',
        icon: <GlobalOutlined />,
        label: 'Web Search',
        path: 'web-search',
        order: 27,
        permission: Permissions.WebSearchAdminRead,
      },
    ],
  },
})

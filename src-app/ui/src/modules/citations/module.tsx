import { BookOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useCitationsStore } from './stores/Citations.store'
// CRITICAL: enable store type declaration merging (registers `Stores.Citations`).
import './types'
// Side-effect: register the "References" knowledge kind on the project page,
// independent of the projects module's load order.
import './project-extension/extension'

const CitationsSettingsPage = lazyWithPreload(() =>
  import('./pages/CitationsSettingsPage').then(m => ({
    default: m.CitationsSettingsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'citations',
    version: '1.0.0',
    description: 'Citation management + verification: a verified CSL-JSON library.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/citations',
      element: CitationsSettingsPage,
      requiresAuth: true,
      permission: Permissions.CitationsUse,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [{ name: 'Citations', store: useCitationsStore }],
  slots: {
    settingsUserPages: [
      {
        id: 'citations',
        icon: <BookOutlined />,
        label: 'Citations',
        path: 'citations',
        order: 35,
        permission: Permissions.CitationsUse,
      },
    ],
  },
})

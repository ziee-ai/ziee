import { Book } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
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
  stores: [],
  slots: {
    settingsUserPages: [
      {
        id: 'citations',
        icon: <Book />,
        label: 'Citations',
        path: 'citations',
        order: 35,
        permission: Permissions.CitationsUse,
      },
    ],
  },
})

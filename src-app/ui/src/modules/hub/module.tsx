import { createModule } from '@/core'
import { AppstoreOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { Permissions } from '@/api-client/types'

// Hub coordination module
// Sub-modules are auto-discovered from hub/modules/**/module.tsx

const HubPage = lazyWithPreload(() =>
  import('./HubPage').then(m => ({ default: m.HubPage })),
)

export default createModule({
  metadata: {
    name: 'hub',
    version: '1.0.0',
    description: 'Hub catalog coordination module',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/hub/:activeTab?',
      element: HubPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'hub',
        icon: <AppstoreOutlined />,
        label: 'Hub',
        path: '/hub',
        order: 30,
        // When adding a new hub submodule, list its ::read here so
        // the sidebar entry only appears for users with access to at
        // least one tab.
        permission: {
          anyOf: [
            Permissions.HubModelsRead,
            Permissions.HubAssistantsRead,
            Permissions.HubMCPServersRead,
          ],
        },
      },
    ],
  },
})

import { createModule } from '@/core'
import { AppstoreOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types/HubTabSlot' // CRITICAL: Import to register slot type

const HubPage = lazyWithPreload(() =>
  import('./HubPage').then(m => ({ default: m.HubPage }))
)

export default createModule({
  metadata: {
    name: 'hub',
    version: '1.0.0',
    description: 'Hub for discovering and installing models, assistants, and MCP servers',
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
        order: 10,
      },
    ],
  },
  initialize: () => {
    console.log('Hub module initialized')
  },
})

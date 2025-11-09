import { createModule } from '@/core'
import { AppstoreOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const HubPage = lazyWithPreload(() => import('./HubPage'))

export default createModule({
  metadata: {
    name: 'hub',
    version: '1.0.0',
    description: 'Hub module for extensions and integrations',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/hub',
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

import { createModule } from '@/core'
import { SettingOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const SettingsLayout = lazyWithPreload(() => import('./SettingsLayout'))

export default createModule({
  metadata: {
    name: 'settings',
    version: '1.0.0',
    description: 'Settings module for user preferences',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings',
      element: SettingsLayout,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'settings',
        icon: <SettingOutlined />,
        label: 'Settings',
        path: '/settings',
        order: 100,
      },
    ],
  },
  initialize: () => {
    console.log('Settings module initialized')
  },
})

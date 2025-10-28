import { createModule } from '@/core'
import { SettingOutlined } from '@ant-design/icons'
import SettingsPage from './SettingsPage'
import AppLayout from '@/components/Layout/AppLayout'

export default createModule({
  metadata: {
    name: 'settings',
    version: '1.0.0',
    description: 'Settings module for user preferences',
  },
  routes: [
    {
      path: '/settings',
      element: <SettingsPage />,
      requiresAuth: true,
      layout: AppLayout,
    },
  ],
  sidebar: {
    tools: [
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

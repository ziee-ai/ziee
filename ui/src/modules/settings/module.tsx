import { createModule } from '@/core'
import SettingsPage from './SettingsPage'

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
      layout: 'default',
    },
  ],
  initialize: () => {
    console.log('Settings module initialized')
  },
})

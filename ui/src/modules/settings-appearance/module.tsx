import { createModule } from '@/core'
import { IoMdEye } from 'react-icons/io'
import AppearanceSettings from './AppearanceSettings'
import SettingsLayout from '@/modules/settings/SettingsLayout'

export default createModule({
  metadata: {
    name: 'settings-appearance',
    version: '1.0.0',
    description: 'Appearance settings',
  },
  routes: [
    {
      path: '/settings/appearance',
      element: <AppearanceSettings />,
      requiresAuth: true,
      layout: SettingsLayout,
    },
  ],
  settings: [
    {
      id: 'appearance',
      icon: <IoMdEye />,
      label: 'Appearance',
      path: 'appearance',
      section: 'user',
      order: 20,
    },
  ],
  initialize: () => {
    console.log('Appearance settings module initialized')
  },
})

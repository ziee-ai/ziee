import { createModule } from '@/core'
import { IoMdPerson } from 'react-icons/io'
import GeneralSettings from './GeneralSettings'
import SettingsLayout from '@/modules/settings/SettingsLayout'

export default createModule({
  metadata: {
    name: 'settings-general',
    version: '1.0.0',
    description: 'General user settings',
  },
  routes: [
    {
      path: '/settings/general',
      element: <GeneralSettings />,
      requiresAuth: true,
      layout: SettingsLayout,
    },
  ],
  settings: [
    {
      id: 'general',
      icon: <IoMdPerson />,
      label: 'General',
      path: 'general',
      section: 'user',
      order: 10,
    },
  ],
  initialize: () => {
    console.log('General settings module initialized')
  },
})

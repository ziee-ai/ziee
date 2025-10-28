import { createModule } from '@/core'
import { IoMdLock } from 'react-icons/io'
import PrivacySettings from './PrivacySettings'
import SettingsLayout from '@/modules/settings/SettingsLayout'

export default createModule({
  metadata: {
    name: 'settings-privacy',
    version: '1.0.0',
    description: 'Privacy settings',
  },
  routes: [
    {
      path: '/settings/privacy',
      element: <PrivacySettings />,
      requiresAuth: true,
      layout: SettingsLayout,
    },
  ],
  settings: [
    {
      id: 'privacy',
      icon: <IoMdLock />,
      label: 'Privacy',
      path: 'privacy',
      section: 'user',
      order: 30,
    },
  ],
  initialize: () => {
    console.log('Privacy settings module initialized')
  },
})

import { createModule } from '@/core'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import HardwareSettings from './HardwareSettings'
import { HardwareMonitor } from './HardwareMonitor'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useHardwareStore } from './store'
import './types' // Import type augmentation

export default createModule({
  metadata: {
    name: 'hardware',
    version: '1.0.0',
    description: 'Hardware monitoring and information',
  },
  routes: [
    {
      path: '/settings/hardware',
      element: <HardwareSettings />,
      requiresAuth: true,
      layout: SettingsLayout,
    },
    {
      path: '/hardware-monitor',
      element: <HardwareMonitor />,
      requiresAuth: true,
    },
  ],
  stores: [
    {
      name: 'Hardware',
      store: useHardwareStore,
    },
  ],
  settings: [
    {
      id: 'hardware',
      icon: <MdOutlineMonitorHeart />,
      label: 'Hardware',
      path: 'hardware',
      section: 'admin',
      order: 30,
    },
  ],
  initialize: () => {
    console.log('Hardware module initialized')
  },
  cleanup: () => {
    console.log('Hardware module cleanup')
  },
})

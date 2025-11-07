import { createModule } from '@/core'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useHardwareStore } from './Hardware.store'
import './types'
import { BlankLayout } from '@/components/Layout/BlankLayout.tsx' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const HardwareSettings = lazyWithPreload(() => import('./HardwareSettings'))
const HardwareMonitor = lazyWithPreload(() => import('./HardwareMonitor').then(m => ({ default: m.HardwareMonitor })))

export default createModule({
  metadata: {
    name: 'hardware',
    version: '1.0.0',
    description: 'Hardware monitoring and information',
  },
  routes: [
    {
      path: '/settings/hardware',
      element: HardwareSettings,
      requiresAuth: true,
      layout: SettingsLayout,
    },
    {
      path: '/hardware-monitor',
      element: HardwareMonitor,
      requiresAuth: true,
      layout: BlankLayout,
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

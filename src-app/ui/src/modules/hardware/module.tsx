import { createModule } from '@/core'
import { MdOutlineMonitorHeart } from 'react-icons/md'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useHardwareStore } from '@/modules/hardware/Hardware.store'
import '@/modules/hardware/types'
import { BlankLayout } from '@/modules/layouts/blank' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const HardwareSettings = lazyWithPreload(() => import('./HardwareSettings'))
const HardwareMonitor = lazyWithPreload(() =>
  import('./HardwareMonitor').then(m => ({ default: m.HardwareMonitor })),
)

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
      layout: SettingsLayoutDef,
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
  slots: {
    settingsAdminPages: [
      {
        id: 'hardware',
        icon: <MdOutlineMonitorHeart />,
        label: 'Hardware',
        path: 'hardware',
        order: 30,
      },
    ],
  },
  initialize: () => {
    console.log('Hardware module initialized')
  },
  cleanup: () => {
    console.log('Hardware module cleanup')
  },
})

import { createModule } from '@ziee/framework'
import { useAppLayoutStore } from '@/modules/layouts/app-layout/AppLayout.store'
import '@/modules/layouts/app-layout/types'

export default createModule({
  metadata: {
    name: 'layout',
    version: '1.0.0',
    description: 'Core layout and UI state management',
  },
  routes: [],
  stores: [
    {
      name: 'AppLayout',
      store: useAppLayoutStore,
    },
  ],
  initialize: () => {
    console.log('Layout module initialized')
  },
})

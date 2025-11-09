import { createModule } from '@/core'
import { useAppLayoutStore } from './AppLayout.store'
import './types'

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

import { createModule } from '@ziee/framework'
import '@/modules/layouts/app-layout/types'

export default createModule({
  metadata: {
    name: 'layout',
    version: '1.0.0',
    description: 'Core layout and UI state management',
  },
  routes: [],
  stores: [
  ],
  initialize: () => {
    console.log('Layout module initialized')
  },
})

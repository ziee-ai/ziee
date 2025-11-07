import { createModule } from '@/core'
import { useConfigClientStore } from './ConfigClient.store'

export default createModule({
  metadata: {
    name: 'config-client',
    version: '1.0.0',
    description: 'Client-side configuration management',
  },
  routes: [],
  stores: [
    {
      name: 'ConfigClient',
      store: useConfigClientStore,
    },
  ],
  initialize: () => {
    console.log('Config-client module initialized')
  },
})

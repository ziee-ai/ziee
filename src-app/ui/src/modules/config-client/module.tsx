import { createModule } from '@ziee/framework'

export default createModule({
  metadata: {
    name: 'config-client',
    version: '1.0.0',
    description: 'Client-side configuration management',
  },
  routes: [],
  stores: [
  ],
  initialize: () => {
    console.log('Config-client module initialized')
  },
})

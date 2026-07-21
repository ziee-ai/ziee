import { createModule } from '@ziee/framework'
import { useConfigClientStore } from '@/modules/config-client/configClient'

export default createModule({
  metadata: {
    name: 'config-client',
    version: '1.0.0',
    description: 'Client-side configuration management',
  },
  routes: [],
  // BOOT-CRITICAL: the theme is applied at first paint from ConfigClient
  // (ThemeProvider, root). It must be eagerly registered — like Auth — so
  // `Stores.ConfigClient`/the handle resolves before any lazy consumer loads.
  // Do NOT lazify this one.
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

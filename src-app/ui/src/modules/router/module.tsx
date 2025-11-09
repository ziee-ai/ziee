import { createModule } from '@/core'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useRoutesStore } from './stores'
import type { AppModule } from '@/core/module-system/types'
import './types' // CRITICAL: Enable type merging for CreateModuleOptions
import './stores/types' // CRITICAL: Enable type merging for Stores

// Lazy load RouterComponent
const RouterComponent = lazyWithPreload(() => import('./components/RouterComponent').then(m => ({ default: m.RouterComponent })))

export default createModule({
  metadata: {
    name: 'router',
    version: '1.0.0',
    description: 'Provides routing infrastructure and layout management',
  },

  dependencies: [], // No dependencies - loads first

  stores: [
    {
      name: 'Routes',
      store: useRoutesStore,
    },
  ],

  components: [
    {
      id: 'router',
      component: RouterComponent,
      order: 0, // Render first!
    },
  ],

  routes: [], // Router module doesn't register routes itself

  /**
   * Hook called when any module is registered.
   * Checks if the module has routes and adds them to Routes store.
   */
  onModuleRegister: (module: AppModule) => {
    // Check if module has routes via type assertion
    // (routes field is added via declaration merging)
    const moduleWithRoutes = module as AppModule & { routes?: any[] }

    if (moduleWithRoutes.routes && moduleWithRoutes.routes.length > 0) {
      console.log(`🛣️  Router: Collecting ${moduleWithRoutes.routes.length} route(s) from module: ${module.metadata.name}`)
      useRoutesStore.getState().addRoutes(moduleWithRoutes.routes)
    }
  },
})

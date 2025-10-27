import { create } from 'zustand'
import type { RouteConfig, AppModule } from './types'
import { createStoreProxy } from '../stores'

interface RouterState {
  routes: RouteConfig[]
  modules: AppModule[]
  stores: Record<string, any>
  registerModule: (module: AppModule) => void
  initializeModules: () => void
}

export const useRouterStore = create<RouterState>((set, get) => ({
  routes: [],
  modules: [],
  stores: {},

  registerModule: (module: AppModule) => {
    set((state) => {
      // Check if module is already registered
      if (state.modules.some((m) => m.metadata.name === module.metadata.name)) {
        console.warn(`Module ${module.metadata.name} is already registered`)
        return state
      }

      // Register the module
      const newModules = [...state.modules, module]

      // Get routes from the module
      const moduleRoutes = module.registerRoutes()
      const newRoutes = [...state.routes, ...moduleRoutes]

      // Get stores from the module
      const newStores = { ...state.stores }
      if (module.registerStores) {
        const storeRegistrations = module.registerStores()
        storeRegistrations.forEach((reg) => {
          newStores[reg.name] = createStoreProxy(reg.store)
        })
        console.log(`Registered module: ${module.metadata.name}`, {
          routes: moduleRoutes.length,
          stores: storeRegistrations.length,
        })
      } else {
        console.log(`Registered module: ${module.metadata.name}`, {
          routes: moduleRoutes.length,
        })
      }

      return {
        modules: newModules,
        routes: newRoutes,
        stores: newStores,
      }
    })
  },

  initializeModules: () => {
    const { modules } = get()

    for (const module of modules) {
      if (module.initialize) {
        try {
          const result = module.initialize()
          // If initialize returns a promise, handle it but don't await
          if (result instanceof Promise) {
            result
              .then(() => console.log(`Initialized module: ${module.metadata.name}`))
              .catch((error) => console.error(`Failed to initialize module ${module.metadata.name}:`, error))
          } else {
            console.log(`Initialized module: ${module.metadata.name}`)
          }
        } catch (error) {
          console.error(`Failed to initialize module ${module.metadata.name}:`, error)
        }
      }
    }
  },
}))

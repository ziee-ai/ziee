import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { routesSeam } from '@ziee/framework'
import type { RouteConfig } from '@/modules/router/types'

const RoutesDef = defineStore('Routes', {
  state: {
    routes: [] as RouteConfig<any>[],
  },
  actions: set => ({
    addRoutes: (routes: RouteConfig<any>[]) => {
      set(state => ({ routes: [...state.routes, ...routes] }))
    },
  }),
})

export const useRoutesStore = RoutesDef.store
export const Routes = registerLazyStore(RoutesDef)

// SEAM: inject into the SDK router/prefetch (replaces the old global Stores.Routes).
routesSeam.set(Routes as unknown as { routes: unknown[] })

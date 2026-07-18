import { defineStore } from '@ziee/framework/store-kit'
import type { RouteConfig } from '@/modules/router/types'

export const Routes = defineStore('Routes', {
  state: {
    routes: [] as RouteConfig<any>[],
  },
  actions: set => ({
    addRoutes: (routes: RouteConfig<any>[]) => {
      set(state => ({ routes: [...state.routes, ...routes] }))
    },
  }),
})

export const useRoutesStore = Routes.store

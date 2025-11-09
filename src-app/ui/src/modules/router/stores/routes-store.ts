import { create } from 'zustand'
import type { RouteConfig } from '../types'

interface RoutesState {
  routes: RouteConfig<any>[]
  addRoutes: (routes: RouteConfig<any>[]) => void
}

export const useRoutesStore = create<RoutesState>((set) => ({
  routes: [],

  addRoutes: (routes: RouteConfig<any>[]) => {
    set(state => ({
      routes: [...state.routes, ...routes],
    }))
  },
}))

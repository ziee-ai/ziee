import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import type { StoreProxy } from '@ziee/framework/stores'
import { appState, type AppState } from './state'
import type { Actions } from './actions.gen'

const AppDef = defineStore<AppState, Actions>('App', {
  state: appState,
  actions: import.meta.glob('./actions/*.ts'),
})

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    App: StoreProxy<ReturnType<typeof AppDef.store.getState>>
  }
}

export const App = registerLazyStore(AppDef)
export const useAppStore = AppDef.store

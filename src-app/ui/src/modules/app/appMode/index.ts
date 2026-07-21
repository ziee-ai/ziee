import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import type { StoreProxy } from '@ziee/framework/stores'
import { appModeState, type AppModeState } from './state'
import type { Actions } from './actions.gen'

const _store = defineStore<AppModeState, Actions>('AppMode', {
  immer: true,
  state: appModeState,
  actions: import.meta.glob('./actions/*.ts'),
})

// `AppMode` is the raw defineStore handle — exported so gallery code can reach
// `.store.setState()` (the proxy from registerLazyStore doesn't expose `.store`).
export const AppMode = _store
export const useAppModeStore = _store.store

registerLazyStore(_store)

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AppMode: StoreProxy<AppModeState>
  }
}

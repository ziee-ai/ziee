import type { StoreProxy } from '@ziee/framework/stores'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'

/**
 * Portable "what kind of build is this" flag for core UI code that branches on
 * multi-user (web) vs single-admin (desktop) semantics without importing the
 * desktop-only platform helpers. Default true (web); the desktop bootstrap flips
 * it to false at startup.
 */
interface AppModeState {
  multiUserMode: boolean
  setMultiUserMode: (value: boolean) => void
}

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AppMode: StoreProxy<AppModeState>
  }
}

const AppModeDef = defineStore('AppMode', {
  state: { multiUserMode: true },
  actions: set => ({
    setMultiUserMode: (value: boolean) => set({ multiUserMode: value }),
  }),
})

export const useAppModeStore = AppModeDef.store

export const AppMode = registerLazyStore(AppModeDef)

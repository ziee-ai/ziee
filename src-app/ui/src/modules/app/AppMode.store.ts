import type { StoreProxy } from '@/core/stores'
import { defineStore } from '@/core/store-kit'

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

declare module '../../core/stores' {
  interface RegisteredStores {
    AppMode: StoreProxy<AppModeState>
  }
}

export const AppMode = defineStore('AppMode', {
  state: { multiUserMode: true },
  actions: set => ({
    setMultiUserMode: (value: boolean) => set({ multiUserMode: value }),
  }),
})

export const useAppModeStore = AppMode.store

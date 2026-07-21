import type { StoreProxy } from '@ziee/framework/stores'
import type { useSessionSettingsStore } from './sessionSettings'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    SessionSettings: StoreProxy<ReturnType<typeof useSessionSettingsStore.getState>>
  }
}

export {}

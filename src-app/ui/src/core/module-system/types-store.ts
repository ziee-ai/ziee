import type { StoreProxy } from '@/core/stores'
import type { useModuleSystemStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    ModuleSystem: StoreProxy<ReturnType<typeof useModuleSystemStore.getState>>
  }
}

export {}

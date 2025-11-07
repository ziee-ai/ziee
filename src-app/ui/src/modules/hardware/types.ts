import type { StoreProxy } from '@/core/stores'
import type { useHardwareStore } from './Hardware.store'

// Augment the RegisteredStores interface to add Hardware store
declare module '@/core/stores' {
  interface RegisteredStores {
    Hardware: StoreProxy<ReturnType<typeof useHardwareStore.getState>>
  }
}

export {}

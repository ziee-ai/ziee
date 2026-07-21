import type { StoreProxy } from '@ziee/framework/stores'
import type { useHardwareStore } from '@/modules/hardware/hardware'

// Augment the RegisteredStores interface to add Hardware store
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Hardware: StoreProxy<ReturnType<typeof useHardwareStore.getState>>
  }
}

export {}

import type { StoreProxy } from '@/core/stores'
import type { useCitationsStore } from './stores/Citations.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Citations: StoreProxy<ReturnType<typeof useCitationsStore.getState>>
  }
}

export {}

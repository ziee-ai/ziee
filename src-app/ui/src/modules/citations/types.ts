import type { StoreProxy } from '@ziee/framework/stores'
import type { useCitationsStore } from './stores/Citations.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Citations: StoreProxy<ReturnType<typeof useCitationsStore.getState>>
  }
}

export {}

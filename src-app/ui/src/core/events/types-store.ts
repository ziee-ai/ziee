import type { StoreProxy } from '@/core/stores'
import type { useEventBusStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    EventBus: StoreProxy<ReturnType<typeof useEventBusStore.getState>>
  }
}

export {}

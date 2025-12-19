import type { StoreProxy } from '@/core/stores'
import type { useEventBusStore } from '@/core/events/store'

declare module '@/core/stores' {
  interface RegisteredStores {
    EventBus: StoreProxy<ReturnType<typeof useEventBusStore.getState>>
  }
}

export {}

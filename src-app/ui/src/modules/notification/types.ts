import type { StoreProxy } from '@/core/stores'

import type { useNotificationsStore } from './stores/Notifications.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Notifications: StoreProxy<ReturnType<typeof useNotificationsStore.getState>>
  }
}

export {}

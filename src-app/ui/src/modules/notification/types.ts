import type { StoreProxy } from '@ziee/framework/stores'

import type { useNotificationsStore } from './stores/Notifications.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Notifications: StoreProxy<ReturnType<typeof useNotificationsStore.getState>>
  }
}

export {}

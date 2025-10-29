import type { StoreProxy } from '@/core/stores'
import type { useUsersStore, useUserGroupsStore } from './store'

// Augment the RegisteredStores interface to add Users and UserGroups stores
declare module '@/core/stores' {
  interface RegisteredStores {
    Users: StoreProxy<ReturnType<typeof useUsersStore.getState>>
    UserGroups: StoreProxy<ReturnType<typeof useUserGroupsStore.getState>>
  }
}

export {}

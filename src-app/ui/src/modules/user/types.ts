import type { StoreProxy } from '@/core/stores'
import type { useUsersStore, useUserGroupsStore } from './stores'
import type { useUserGroupDrawerStore } from './components/EditUserGroupDrawer.store'
import type { useGroupMembersDrawerStore } from './components/GroupMembersDrawer.store'

// Augment the RegisteredStores interface to add Users and UserGroups stores
declare module '@/core/stores' {
  interface RegisteredStores {
    Users: StoreProxy<ReturnType<typeof useUsersStore.getState>>
    UserGroups: StoreProxy<ReturnType<typeof useUserGroupsStore.getState>>
    UserGroupDrawer: StoreProxy<ReturnType<typeof useUserGroupDrawerStore.getState>>
    GroupMembersDrawer: StoreProxy<ReturnType<typeof useGroupMembersDrawerStore.getState>>
  }
}

export {}

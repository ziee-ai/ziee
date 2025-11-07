import type { StoreProxy } from '@/core/stores'
import type { useUsersStore, useUserGroupsStore } from '../stores'
import type { useUserGroupDrawerStore } from '../components/group/EditUserGroupDrawer.store'
import type { useGroupMembersDrawerStore } from '../components/group/GroupMembersDrawer.store'
import type { useCreateUserDrawerStore } from '../components/user/CreateUserDrawer.store'
import type { useEditUserDrawerStore } from '../components/user/EditUserDrawer.store'
import type { useResetPasswordDrawerStore } from '../components/user/ResetPasswordDrawer.store'
import type { useUserGroupsDrawerStore } from '../components/user/UserGroupsDrawer.store'
import type { useAssignGroupDrawerStore } from '../components/user/AssignGroupDrawer.store'

// Augment the RegisteredStores interface to add Users and UserGroups stores
declare module '@/core/stores' {
  interface RegisteredStores {
    Users: StoreProxy<ReturnType<typeof useUsersStore.getState>>
    UserGroups: StoreProxy<ReturnType<typeof useUserGroupsStore.getState>>
    EditUserGroupDrawer: StoreProxy<ReturnType<typeof useUserGroupDrawerStore.getState>>
    GroupMembersDrawer: StoreProxy<ReturnType<typeof useGroupMembersDrawerStore.getState>>
    CreateUserDrawer: StoreProxy<ReturnType<typeof useCreateUserDrawerStore.getState>>
    EditUserDrawer: StoreProxy<ReturnType<typeof useEditUserDrawerStore.getState>>
    ResetPasswordDrawer: StoreProxy<ReturnType<typeof useResetPasswordDrawerStore.getState>>
    UserGroupsDrawer: StoreProxy<ReturnType<typeof useUserGroupsDrawerStore.getState>>
    AssignGroupDrawer: StoreProxy<ReturnType<typeof useAssignGroupDrawerStore.getState>>
  }
}

export * from './GroupWidget'

import type { StoreProxy } from '@ziee/framework/stores'
// Direct type import (erased) — keeps the `UsersStore` augmentation without
// re-tethering the lazy Users store through the barrel.
import type { useUsersStore } from '@/modules/user/stores/users'
import type { useUserGroupsStore } from '@/modules/user/stores'
import type { useUserGroupDrawerStore } from '@/modules/user/components/group/editUserGroupDrawer'
import type { useGroupMembersDrawerStore } from '@/modules/user/components/group/groupMembersDrawer'
import type { useCreateUserDrawerStore } from '@/modules/user/components/user/createUserDrawer'
import type { useEditUserDrawerStore } from '@/modules/user/components/user/editUserDrawer'
import type { useResetPasswordDrawerStore } from '@/modules/user/components/user/resetPasswordDrawer'
import type { useUserGroupsDrawerStore } from '@/modules/user/components/user/userGroupsDrawer'
import type { useAssignGroupDrawerStore } from '@/modules/user/components/user/assignGroupDrawer'

// Augment the RegisteredStores interface to add Users and UserGroups stores
declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Users: StoreProxy<ReturnType<typeof useUsersStore.getState>>
    UserGroups: StoreProxy<ReturnType<typeof useUserGroupsStore.getState>>
    EditUserGroupDrawer: StoreProxy<
      ReturnType<typeof useUserGroupDrawerStore.getState>
    >
    GroupMembersDrawer: StoreProxy<
      ReturnType<typeof useGroupMembersDrawerStore.getState>
    >
    CreateUserDrawer: StoreProxy<
      ReturnType<typeof useCreateUserDrawerStore.getState>
    >
    EditUserDrawer: StoreProxy<
      ReturnType<typeof useEditUserDrawerStore.getState>
    >
    ResetPasswordDrawer: StoreProxy<
      ReturnType<typeof useResetPasswordDrawerStore.getState>
    >
    UserGroupsDrawer: StoreProxy<
      ReturnType<typeof useUserGroupsDrawerStore.getState>
    >
    AssignGroupDrawer: StoreProxy<
      ReturnType<typeof useAssignGroupDrawerStore.getState>
    >
  }
}

export * from './GroupWidget'

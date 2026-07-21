import { User as UserIcon, Users as UsersIcon } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useUserGroupDrawerStore } from '@/modules/user/components/group/editUserGroupDrawer'
import { useGroupMembersDrawerStore } from '@/modules/user/components/group/groupMembersDrawer'
import { useAssignGroupDrawerStore } from '@/modules/user/components/user/assignGroupDrawer'
import { useCreateUserDrawerStore } from '@/modules/user/components/user/createUserDrawer'
import { useEditUserDrawerStore } from '@/modules/user/components/user/editUserDrawer'
import { useResetPasswordDrawerStore } from '@/modules/user/components/user/resetPasswordDrawer'
import { useUserGroupsDrawerStore } from '@/modules/user/components/user/UserGroupsDrawer.store'
import { useUserGroupsStore } from '@/modules/user/stores'
// NOTE: the `Users` store is NOT imported/registered here — it is whole-store-lazy
// (self-registers from its own chunk when a consumer imports it). See Users.store.ts.
import '@/modules/user/types' // Import type augmentation
import '@/modules/user/types/GroupWidget' // Register userGroup widget slot
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const UsersSettings = lazyWithPreload(() =>
  import('./components/user/UsersSettings').then(m => ({
    default: m.UsersSettings,
  })),
)
const UserGroupsSettings = lazyWithPreload(() =>
  import('./components/group/UserGroupsSettings').then(m => ({
    default: m.UserGroupsSettings,
  })),
)

export default createModule({
  metadata: {
    name: 'user',
    version: '1.0.0',
    description: 'User and user group management',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/users',
      element: UsersSettings,
      requiresAuth: true,
      permission: Permissions.UsersRead,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/user-groups',
      element: UserGroupsSettings,
      requiresAuth: true,
      permission: Permissions.GroupsRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'UserGroups',
      store: useUserGroupsStore,
    },
    {
      name: 'EditUserGroupDrawer',
      store: useUserGroupDrawerStore,
    },
    {
      name: 'GroupMembersDrawer',
      store: useGroupMembersDrawerStore,
    },
    {
      name: 'CreateUserDrawer',
      store: useCreateUserDrawerStore,
    },
    {
      name: 'EditUserDrawer',
      store: useEditUserDrawerStore,
    },
    {
      name: 'ResetPasswordDrawer',
      store: useResetPasswordDrawerStore,
    },
    {
      name: 'UserGroupsDrawer',
      store: useUserGroupsDrawerStore,
    },
    {
      name: 'AssignGroupDrawer',
      store: useAssignGroupDrawerStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'users',
        icon: <UserIcon />,
        label: 'Users',
        path: 'users',
        order: 10,
        permission: Permissions.UsersRead,
      },
      {
        id: 'user-groups',
        icon: <UsersIcon />,
        label: 'User Groups',
        path: 'user-groups',
        order: 11,
        permission: Permissions.GroupsRead,
      },
    ],
  },
  initialize: () => {
    console.log('User module initialized')
  },
  cleanup: () => {
    console.log('User module cleanup')
  },
})

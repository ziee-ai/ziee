import { createModule } from '@/core'
import { UserOutlined, TeamOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useUsersStore, useUserGroupsStore } from './stores'
import { useUserGroupDrawerStore } from './components/group/EditUserGroupDrawer.store'
import { useGroupMembersDrawerStore } from './components/group/GroupMembersDrawer.store'
import { useCreateUserDrawerStore } from './components/user/CreateUserDrawer.store'
import { useEditUserDrawerStore } from './components/user/EditUserDrawer.store'
import { useResetPasswordDrawerStore } from './components/user/ResetPasswordDrawer.store'
import { useUserGroupsDrawerStore } from './components/user/UserGroupsDrawer.store'
import { useAssignGroupDrawerStore } from './components/user/AssignGroupDrawer.store'
import './types' // Import type augmentation
import './types/GroupWidget' // Register userGroup widget slot
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
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/user-groups',
      element: UserGroupsSettings,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'Users',
      store: useUsersStore,
    },
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
        icon: <UserOutlined />,
        label: 'Users',
        path: 'users',
        order: 10,
      },
      {
        id: 'user-groups',
        icon: <TeamOutlined />,
        label: 'User Groups',
        path: 'user-groups',
        order: 11,
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

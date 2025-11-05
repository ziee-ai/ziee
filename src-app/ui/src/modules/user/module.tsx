import { createModule } from '@/core'
import { UserOutlined, TeamOutlined } from '@ant-design/icons'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useUsersStore, useUserGroupsStore } from './store'
import './types' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const UsersSettings = lazyWithPreload(() => import('./components/UsersSettings').then(m => ({ default: m.UsersSettings })))
const UserGroupsSettings = lazyWithPreload(() => import('./components/UserGroupsSettings').then(m => ({ default: m.UserGroupsSettings })))

export default createModule({
  metadata: {
    name: 'user',
    version: '1.0.0',
    description: 'User and user group management',
  },
  routes: [
    {
      path: '/settings/users',
      element: UsersSettings,
      requiresAuth: true,
      layout: SettingsLayout,
    },
    {
      path: '/settings/user-groups',
      element: UserGroupsSettings,
      requiresAuth: true,
      layout: SettingsLayout,
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
  ],
  settings: [
    {
      id: 'users',
      icon: <UserOutlined />,
      label: 'Users',
      path: 'users',
      section: 'admin',
      order: 10,
    },
    {
      id: 'user-groups',
      icon: <TeamOutlined />,
      label: 'User Groups',
      path: 'user-groups',
      section: 'admin',
      order: 11,
    },
  ],
  initialize: () => {
    console.log('User module initialized')
  },
  cleanup: () => {
    console.log('User module cleanup')
  },
})

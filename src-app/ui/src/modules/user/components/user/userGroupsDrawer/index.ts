import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { userGroupsDrawerState, type UserGroupsDrawerState } from './state'
import type { Actions } from './actions.gen'

const UserGroupsDrawerDef = defineStore<UserGroupsDrawerState, Actions>('UserGroupsDrawer', {
  immer: true,
  state: userGroupsDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    on('user.deleted', event => {
      if (get().user?.id === event.data.userId) actions.closeUserGroupsDrawer()
    })
  },
})
export const UserGroupsDrawer = registerLazyStore(UserGroupsDrawerDef)
export const useUserGroupsDrawerStore = UserGroupsDrawerDef.store

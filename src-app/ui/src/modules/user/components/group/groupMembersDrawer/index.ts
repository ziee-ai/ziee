import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupMembersDrawerState, type GroupMembersDrawerState } from './state'
import type { Actions } from './actions.gen'

const GroupMembersDrawerDef = defineStore<GroupMembersDrawerState, Actions>('GroupMembersDrawer', {
  immer: true,
  state: groupMembersDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    on('group.updated', event => {
      if (get().selectedGroup?.id === event.data.group.id) {
        set({ selectedGroup: event.data.group })
      }
    })
    on('group.deleted', event => {
      if (get().selectedGroup?.id === event.data.groupId) {
        void actions.closeGroupMembersDrawer()
      }
    })
  },
})

export const GroupMembersDrawer = registerLazyStore(GroupMembersDrawerDef)
export const useGroupMembersDrawerStore = GroupMembersDrawerDef.store

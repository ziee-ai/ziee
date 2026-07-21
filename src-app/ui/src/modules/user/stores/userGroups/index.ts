import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { userGroupsState, type UserGroupsState } from './state'
import type { Actions } from './actions.gen'

const UserGroupsDef = defineStore<UserGroupsState, Actions>('UserGroups', {
  immer: false,
  state: userGroupsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    on('group.created', event => {
      set(state => ({ groups: [...state.groups, event.data.group], total: state.total + 1 }))
    })
    on('group.updated', event => {
      set(state => ({
        groups: state.groups.map(g => (g.id === event.data.group.id ? event.data.group : g)),
      }))
    })
    on('group.deleted', event => {
      set(state => ({
        groups: state.groups.filter(g => g.id !== event.data.groupId),
        total: state.total - 1,
      }))
    })
    on('group.member_added', async event => {
      if (get().currentGroupId === event.data.groupId) {
        await actions.loadUserGroupMembers(event.data.groupId)
      }
    })
    on('group.member_removed', event => {
      const { groupId, userId } = event.data
      if (get().currentGroupId === groupId) {
        set(state => ({
          currentGroupMembers: state.currentGroupMembers.filter(m => m.id !== userId),
        }))
      }
    })
    on('user.deleted', event => {
      set(state => ({
        currentGroupMembers: state.currentGroupMembers.filter(m => m.id !== event.data.userId),
      }))
    })
    // Remote sync: loadUserGroups self-gates on GroupsRead.
    const reload = () => void actions.loadUserGroups()
    on('sync:group', reload)
    on('sync:reconnect', reload)
    void actions.loadUserGroups()
  },
})

export const UserGroups = registerLazyStore(UserGroupsDef)
export const useUserGroupsStore = UserGroupsDef.store

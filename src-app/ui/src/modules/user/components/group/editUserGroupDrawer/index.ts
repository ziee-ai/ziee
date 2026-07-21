import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { editUserGroupDrawerState, type EditUserGroupDrawerState } from './state'
import type { Actions } from './actions.gen'

const EditUserGroupDrawerDef = defineStore<EditUserGroupDrawerState, Actions>(
  'EditUserGroupDrawer',
  {
    immer: true,
    state: editUserGroupDrawerState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, get, set, actions }) => {
      on('group.updated', event => {
        if (get().editingGroup?.id === event.data.group.id)
          set({ editingGroup: event.data.group })
      })
      on('group.deleted', event => {
        if (get().editingGroup?.id === event.data.groupId)
          actions.closeUserGroupDrawer()
      })
    },
  },
)

export const EditUserGroupDrawer = registerLazyStore(EditUserGroupDrawerDef)
export const useUserGroupDrawerStore = EditUserGroupDrawerDef.store

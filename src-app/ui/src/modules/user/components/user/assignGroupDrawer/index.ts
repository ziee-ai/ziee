import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { assignGroupDrawerState, type AssignGroupDrawerState } from './state'
import type { Actions } from './actions.gen'

const AssignGroupDrawerDef = defineStore<AssignGroupDrawerState, Actions>('AssignGroupDrawer', {
  immer: true,
  state: assignGroupDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, actions }) => {
    on('user.deleted', event => {
      if (get().user?.id === event.data.userId) actions.closeAssignGroupDrawer()
    })
  },
})
export const AssignGroupDrawer = registerLazyStore(AssignGroupDrawerDef)
export const useAssignGroupDrawerStore = AssignGroupDrawerDef.store

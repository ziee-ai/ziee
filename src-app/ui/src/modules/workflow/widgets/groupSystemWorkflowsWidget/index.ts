import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { groupSystemWorkflowsWidgetState, type GroupSystemWorkflowsWidgetState } from './state'
import type { Actions } from './actions.gen'

const GroupSystemWorkflowsWidgetDef = defineStore<GroupSystemWorkflowsWidgetState, Actions>('GroupSystemWorkflowsWidget', {
  immer: true,
  state: groupSystemWorkflowsWidgetState,
  actions: import.meta.glob('./actions/*.ts'),
})

export const GroupSystemWorkflowsWidget = registerLazyStore(GroupSystemWorkflowsWidgetDef)
export const useGroupSystemWorkflowsWidgetStore = GroupSystemWorkflowsWidgetDef.store

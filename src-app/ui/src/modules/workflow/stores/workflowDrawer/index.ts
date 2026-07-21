import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { workflowDrawerState } from './state'
import type { WorkflowDrawerState } from './state'
import type { Actions } from './actions.gen'

const WorkflowDrawerDef = defineStore<WorkflowDrawerState, Actions>('WorkflowDrawer', {
  immer: true,
  state: workflowDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const WorkflowDrawer = registerLazyStore(WorkflowDrawerDef)
export const useWorkflowDrawerStore = WorkflowDrawerDef.store

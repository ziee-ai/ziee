import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { workflowState, type WorkflowState } from './state'
import type { Actions } from './actions.gen'

const WorkflowDef = defineStore<WorkflowState, Actions>('Workflow', {
  immer: true,
  state: workflowState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadWorkflows()
    on('sync:workflow', reload)
    on('sync:reconnect', reload)
    void actions.loadWorkflows()
  },
})
export const Workflow = registerLazyStore(WorkflowDef)
export const useWorkflowStore = WorkflowDef.store

import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { systemWorkflowState, type SystemWorkflowState } from './state'
import type { Actions } from './actions.gen'

const SystemWorkflowDef = defineStore<SystemWorkflowState, Actions>('SystemWorkflow', {
  immer: true,
  state: systemWorkflowState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadSystemWorkflows()
    on('sync:workflow_system', reload)
    on('sync:reconnect', reload)
    void actions.loadSystemWorkflows()
  },
})
export const SystemWorkflow = registerLazyStore(SystemWorkflowDef)
export const useSystemWorkflowStore = SystemWorkflowDef.store

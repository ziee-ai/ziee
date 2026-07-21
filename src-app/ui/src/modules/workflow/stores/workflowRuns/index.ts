import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { workflowRunsState, type WorkflowRunsState } from './state'
import type { Actions } from './actions.gen'

const WorkflowRunsDef = defineStore<WorkflowRunsState, Actions>(
  'WorkflowRuns',
  {
    immer: true,
    state: workflowRunsState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, get, actions }) => {
      const reload = () => {
        for (const wid of Object.keys(get().runs))
          void actions.loadRuns(wid)
      }
      on('sync:workflow_run', reload)
      on('sync:reconnect', reload)
    },
  },
)
export const WorkflowRuns = registerLazyStore(WorkflowRunsDef)
export const useWorkflowRunsStore = WorkflowRunsDef.store

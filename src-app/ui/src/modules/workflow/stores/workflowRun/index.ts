import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { workflowRunState } from './state'
import type { Actions } from './actions.gen'

export type {
  StepOutputMeta,
  StepArtifactMeta,
  StepProgress,
  RunView,
} from './state'

const WorkflowRunDef = defineStore<typeof workflowRunState, Actions>('WorkflowRun', {
  immer: true,
  state: workflowRunState,
  actions: import.meta.glob('./actions/*.ts'),
})
export const WorkflowRun = registerLazyStore(WorkflowRunDef)
export const useWorkflowRunStore = WorkflowRunDef.store

import type { StoreProxy } from '@/core/stores'
import type {
  useSystemWorkflowStore,
  useWorkflowDrawerStore,
  useWorkflowRunStore,
  useWorkflowStore,
} from '@/modules/workflow/stores'

declare module '@/core/stores' {
  interface RegisteredStores {
    Workflow: StoreProxy<ReturnType<typeof useWorkflowStore.getState>>
    SystemWorkflow: StoreProxy<
      ReturnType<typeof useSystemWorkflowStore.getState>
    >
    WorkflowRun: StoreProxy<ReturnType<typeof useWorkflowRunStore.getState>>
    WorkflowDrawer: StoreProxy<
      ReturnType<typeof useWorkflowDrawerStore.getState>
    >
  }
}

export {}

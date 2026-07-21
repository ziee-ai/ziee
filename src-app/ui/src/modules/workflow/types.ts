import type { StoreProxy } from '@ziee/framework/stores'
import type {
  useSystemWorkflowStore,
  useWorkflowDrawerStore,
  useWorkflowRunStore,
  useWorkflowRunsStore,
  useWorkflowStore,
} from '@/modules/workflow/stores'
import type { useGroupSystemWorkflowsWidgetStore } from '@/modules/workflow/widgets/groupSystemWorkflowsWidget'
import type { useGroupSystemWorkflowsAssignmentStore } from '@/modules/workflow/widgets/GroupSystemWorkflowsAssignmentDrawer.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    Workflow: StoreProxy<ReturnType<typeof useWorkflowStore.getState>>
    SystemWorkflow: StoreProxy<
      ReturnType<typeof useSystemWorkflowStore.getState>
    >
    WorkflowRun: StoreProxy<ReturnType<typeof useWorkflowRunStore.getState>>
    WorkflowRuns: StoreProxy<ReturnType<typeof useWorkflowRunsStore.getState>>
    WorkflowDrawer: StoreProxy<
      ReturnType<typeof useWorkflowDrawerStore.getState>
    >
    GroupSystemWorkflowsWidget: StoreProxy<
      ReturnType<typeof useGroupSystemWorkflowsWidgetStore.getState>
    >
    GroupSystemWorkflowsAssignment: StoreProxy<
      ReturnType<typeof useGroupSystemWorkflowsAssignmentStore.getState>
    >
  }
}

export {}

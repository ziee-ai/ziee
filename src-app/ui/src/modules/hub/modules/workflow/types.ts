import type { StoreProxy } from '@/core/stores'
import type { useHubWorkflowsStore } from '@/modules/hub/modules/workflow/stores/hub-workflows-store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubWorkflows: StoreProxy<ReturnType<typeof useHubWorkflowsStore.getState>>
  }
}

export {}

import type { StoreProxy } from '@ziee/framework/stores'
import type { useHubWorkflowsStore } from '@/modules/hub/modules/workflow/stores/hub-workflows-store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    HubWorkflows: StoreProxy<ReturnType<typeof useHubWorkflowsStore.getState>>
  }
}

export {}

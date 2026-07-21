import type { StoreProxy } from '@ziee/framework/stores'
import type { useHubModelsStore } from '@/modules/hub/modules/llm-models/stores/hub-models-store'
import type { useModelDetailsDrawerStore } from '@/modules/hub/modules/llm-models/components/modelDetailsDrawer'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    HubModels: StoreProxy<ReturnType<typeof useHubModelsStore.getState>>
    ModelDetailsDrawer: StoreProxy<
      ReturnType<typeof useModelDetailsDrawerStore.getState>
    >
  }
}

export {}

import type { StoreProxy } from '@/core/stores'
import type { useHubModelsStore } from '@/modules/hub/modules/llm-models/stores/hub-models-store'
import type { useModelDetailsDrawerStore } from '@/modules/hub/modules/llm-models/components/ModelDetailsDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubModels: StoreProxy<ReturnType<typeof useHubModelsStore.getState>>
    ModelDetailsDrawer: StoreProxy<
      ReturnType<typeof useModelDetailsDrawerStore.getState>
    >
  }
}

export {}

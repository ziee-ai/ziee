import type { StoreProxy } from '@/core/stores'
import type { useHubModelsStore } from './stores/hub-models-store'
import type { useModelDetailsDrawerStore } from './components/ModelDetailsDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubModels: StoreProxy<ReturnType<typeof useHubModelsStore.getState>>
    ModelDetailsDrawer: StoreProxy<ReturnType<typeof useModelDetailsDrawerStore.getState>>
  }
}

export {}

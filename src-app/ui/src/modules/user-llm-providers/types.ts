import type { StoreProxy } from '@ziee/framework/stores'
import type { useUserLlmProvidersStore } from './UserLlmProviders.store'
import type { useModelPickerStore } from './modelPicker'
import type { useUserProviderKeysStore } from './userProviderKeys'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    UserLlmProviders: StoreProxy<ReturnType<typeof useUserLlmProvidersStore.getState>>
    ModelPicker: StoreProxy<ReturnType<typeof useModelPickerStore.getState>>
    UserProviderKeys: StoreProxy<ReturnType<typeof useUserProviderKeysStore.getState>>
  }
}

export {}

import type { StoreProxy } from '@/core/stores'
import type { useUserLlmProvidersStore } from './UserLlmProviders.store'
import type { useModelPickerStore } from './ModelPicker.store'
import type { useUserProviderKeysStore } from './UserProviderKeys.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    UserLlmProviders: StoreProxy<ReturnType<typeof useUserLlmProvidersStore.getState>>
    ModelPicker: StoreProxy<ReturnType<typeof useModelPickerStore.getState>>
    UserProviderKeys: StoreProxy<ReturnType<typeof useUserProviderKeysStore.getState>>
  }
}

export {}

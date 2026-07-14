import type { StoreProxy } from '@ziee/framework/stores'
import type { useHubAssistantsStore } from '@/modules/hub/modules/assistants/stores/hub-assistants-store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    HubAssistants: StoreProxy<ReturnType<typeof useHubAssistantsStore.getState>>
  }
}

export {}

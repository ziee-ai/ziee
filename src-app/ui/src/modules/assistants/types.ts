import type { StoreProxy } from '@/core/stores'
import type { useHubAssistantsStore } from './stores/hub-assistants-store'
import type { useUserAssistantsStore, useTemplateAssistantsStore } from './stores'
import type { useAssistantDrawerStore } from './components/AssistantDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    UserAssistants: StoreProxy<ReturnType<typeof useUserAssistantsStore.getState>>
    TemplateAssistants: StoreProxy<ReturnType<typeof useTemplateAssistantsStore.getState>>
    AssistantDrawer: StoreProxy<ReturnType<typeof useAssistantDrawerStore.getState>>
    HubAssistants: StoreProxy<ReturnType<typeof useHubAssistantsStore.getState>>
  }
}

export {}

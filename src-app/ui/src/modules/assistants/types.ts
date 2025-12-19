import type { StoreProxy } from '@/core/stores'
import type {
  useUserAssistantsStore,
  useTemplateAssistantsStore,
} from '@/modules/assistants/stores'
import type { useAssistantDrawerStore } from '@/modules/assistants/components/AssistantDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    UserAssistants: StoreProxy<
      ReturnType<typeof useUserAssistantsStore.getState>
    >
    TemplateAssistants: StoreProxy<
      ReturnType<typeof useTemplateAssistantsStore.getState>
    >
    AssistantDrawer: StoreProxy<
      ReturnType<typeof useAssistantDrawerStore.getState>
    >
  }
}

export {}

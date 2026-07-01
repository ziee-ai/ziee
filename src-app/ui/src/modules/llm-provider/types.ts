import type { StoreProxy } from '@/core/stores'
import type {
  useLlmProviderStore,
  useLlmModelDownloadStore,
  useAddLocalLlmModelUploadDrawerStore,
  useAddLocalLlmModelDownloadDrawerStore,
  useEditLlmModelDrawerStore,
  useAddRemoteLlmModelDrawerStore,
  useViewDownloadDrawerStore,
  useUploadStore,
} from '@/modules/llm-provider/stores'
import type { useProviderGroupCardStore } from '@/modules/llm-provider/components/ProviderGroupAssignmentCard.store'
import type { useLlmProviderGroupWidgetStore } from '@/modules/llm-provider/widgets/LLMProviderGroupWidget.store'
import type { useLlmProviderDrawerStore } from '@/modules/llm-provider/components/LlmProviderDrawer.store'
import type { useGroupLlmProvidersAssignmentStore } from '@/modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    LlmProvider: StoreProxy<ReturnType<typeof useLlmProviderStore.getState>>
    LlmModelDownload: StoreProxy<
      ReturnType<typeof useLlmModelDownloadStore.getState>
    >
    LlmProviderDrawer: StoreProxy<
      ReturnType<typeof useLlmProviderDrawerStore.getState>
    >
    AddLocalLlmModelUploadDrawer: StoreProxy<
      ReturnType<typeof useAddLocalLlmModelUploadDrawerStore.getState>
    >
    AddLocalLlmModelDownloadDrawer: StoreProxy<
      ReturnType<typeof useAddLocalLlmModelDownloadDrawerStore.getState>
    >
    EditLlmModelDrawer: StoreProxy<
      ReturnType<typeof useEditLlmModelDrawerStore.getState>
    >
    AddRemoteLlmModelDrawer: StoreProxy<
      ReturnType<typeof useAddRemoteLlmModelDrawerStore.getState>
    >
    ViewDownloadDrawer: StoreProxy<
      ReturnType<typeof useViewDownloadDrawerStore.getState>
    >
    LlmModelUpload: StoreProxy<ReturnType<typeof useUploadStore.getState>>
    GroupLlmProvidersAssignment: StoreProxy<
      ReturnType<typeof useGroupLlmProvidersAssignmentStore.getState>
    >
    LlmProviderGroupWidget: StoreProxy<
      ReturnType<typeof useLlmProviderGroupWidgetStore.getState>
    >
    ProviderGroupAssignmentCard: StoreProxy<
      ReturnType<typeof useProviderGroupCardStore.getState>
    >
  }
}

export {}

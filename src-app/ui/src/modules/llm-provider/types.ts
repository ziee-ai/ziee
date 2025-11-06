import type { StoreProxy } from '@/core/stores'
import type {
  useLlmProviderStore,
  useLlmModelDownloadStore,
  useLlmProviderDrawerStore,
  useAddLocalLlmModelUploadDrawerStore,
  useAddLocalLlmModelDownloadDrawerStore,
  useEditLlmModelDrawerStore,
  useAddRemoteLlmModelDrawerStore,
  useViewDownloadDrawerStore,
  useUploadStore,
  useLlmProviderGroupAssignmentStore,
  useProviderGroupAssignmentStore,
  useLlmProviderGroupWidgetStore,
  useProviderGroupCardStore,
} from './stores'

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
    LlmProviderGroupAssignment: StoreProxy<
      ReturnType<typeof useLlmProviderGroupAssignmentStore.getState>
    >
    ProviderGroupAssignment: StoreProxy<
      ReturnType<typeof useProviderGroupAssignmentStore.getState>
    >
    LlmProviderGroupWidget: StoreProxy<
      ReturnType<typeof useLlmProviderGroupWidgetStore.getState>
    >
    ProviderGroupCard: StoreProxy<
      ReturnType<typeof useProviderGroupCardStore.getState>
    >
  }
}

export {}

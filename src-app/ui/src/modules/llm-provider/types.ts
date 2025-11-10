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
} from './stores'
import type { useHubModelsStore } from './stores/hub-models-store'
import type { useProviderGroupCardStore } from './components/ProviderGroupAssignmentCard.store'
import type { useLlmProviderGroupWidgetStore } from './widgets/LLMProviderGroupWidget.store'
import type { useLlmProviderDrawerStore } from './components/LlmProviderDrawer.store'
import type { useGroupLlmProvidersAssignmentStore } from './components/GroupLlmProvidersAssignmentDrawer.store'
import type { useLlmProviderGroupsAssignmentStore } from './components/LlmProviderGroupsAssignmentDrawer.store'
import type { useModelDetailsDrawerStore } from './components/hub/ModelDetailsDrawer.store'

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
    LlmProviderGroupsAssignment: StoreProxy<
      ReturnType<typeof useLlmProviderGroupsAssignmentStore.getState>
    >
    LlmProviderGroupWidget: StoreProxy<
      ReturnType<typeof useLlmProviderGroupWidgetStore.getState>
    >
    ProviderGroupAssignmentCard: StoreProxy<
      ReturnType<typeof useProviderGroupCardStore.getState>
    >
    HubModels: StoreProxy<ReturnType<typeof useHubModelsStore.getState>>
    ModelDetailsDrawer: StoreProxy<
      ReturnType<typeof useModelDetailsDrawerStore.getState>
    >
  }
}

export {}

import { create } from 'zustand'

// ===== Add Local Model Upload Drawer =====
interface AddLocalLlmModelUploadDrawerState {
  open: boolean
  loading: boolean
  providerId: string | null
}

export const useAddLocalLlmModelUploadDrawerStore =
  create<AddLocalLlmModelUploadDrawerState>(() => ({
    open: false,
    loading: false,
    providerId: null,
  }))

export const openAddLocalLlmModelUploadDrawer = (providerId: string) => {
  useAddLocalLlmModelUploadDrawerStore.setState({
    open: true,
    providerId,
  })
}

export const closeAddLocalLlmModelUploadDrawer = () => {
  useAddLocalLlmModelUploadDrawerStore.setState({
    open: false,
    loading: false,
    providerId: null,
  })
}

export const setAddLocalLlmModelUploadDrawerLoading = (loading: boolean) => {
  useAddLocalLlmModelUploadDrawerStore.setState({
    loading,
  })
}

// ===== Add Local Model Download Drawer =====
interface AddLocalLlmModelDownloadDrawerState {
  open: boolean
  loading: boolean
  providerId: string | null
}

export const useAddLocalLlmModelDownloadDrawerStore =
  create<AddLocalLlmModelDownloadDrawerState>(() => ({
    open: false,
    loading: false,
    providerId: null,
  }))

export const openAddLocalLlmModelDownloadDrawer = (providerId: string) => {
  useAddLocalLlmModelDownloadDrawerStore.setState({
    open: true,
    providerId,
  })
}

export const closeAddLocalLlmModelDownloadDrawer = () => {
  useAddLocalLlmModelDownloadDrawerStore.setState({
    open: false,
    loading: false,
    providerId: null,
  })
}

export const setAddLocalLlmModelDownloadDrawerLoading = (loading: boolean) => {
  useAddLocalLlmModelDownloadDrawerStore.setState({
    loading,
  })
}

// ===== Edit LLM Model Drawer (Unified for Local & Remote) =====
interface EditLlmModelDrawerState {
  open: boolean
  loading: boolean
  modelId: string | null
}

export const useEditLlmModelDrawerStore = create<EditLlmModelDrawerState>(
  () => ({
    open: false,
    loading: false,
    modelId: null,
  }),
)

export const openEditLlmModelDrawer = (modelId: string) => {
  useEditLlmModelDrawerStore.setState({
    open: true,
    modelId,
  })
}

export const closeEditLlmModelDrawer = () => {
  useEditLlmModelDrawerStore.setState({
    open: false,
    loading: false,
    modelId: null,
  })
}

export const setEditLlmModelDrawerLoading = (loading: boolean) => {
  useEditLlmModelDrawerStore.setState({
    loading,
  })
}

// ===== Add Remote LLM Model Drawer =====
interface AddRemoteLlmModelDrawerState {
  open: boolean
  loading: boolean
  providerId: string | null
  providerType: string | null
}

export const useAddRemoteLlmModelDrawerStore =
  create<AddRemoteLlmModelDrawerState>(() => ({
    open: false,
    loading: false,
    providerId: null,
    providerType: null,
  }))

export const openAddRemoteLlmModelDrawer = (
  providerId: string,
  providerType: string,
) => {
  useAddRemoteLlmModelDrawerStore.setState({
    open: true,
    providerId,
    providerType,
  })
}

export const closeAddRemoteLlmModelDrawer = () => {
  useAddRemoteLlmModelDrawerStore.setState({
    open: false,
    loading: false,
    providerId: null,
    providerType: null,
  })
}

export const setAddRemoteLlmModelDrawerLoading = (loading: boolean) => {
  useAddRemoteLlmModelDrawerStore.setState({
    loading,
  })
}

// ===== View Download Drawer =====
interface ViewDownloadDrawerState {
  open: boolean
  loading: boolean
  downloadId: string | null
}

export const useViewDownloadDrawerStore = create<ViewDownloadDrawerState>(
  () => ({
    open: false,
    loading: false,
    downloadId: null,
  }),
)

export const openViewDownloadDrawer = (downloadId: string) => {
  useViewDownloadDrawerStore.setState({
    open: true,
    downloadId,
  })
}

export const closeViewDownloadDrawer = () => {
  useViewDownloadDrawerStore.setState({
    open: false,
    loading: false,
    downloadId: null,
  })
}

export const setViewDownloadDrawerLoading = (loading: boolean) => {
  useViewDownloadDrawerStore.setState({
    loading,
  })
}

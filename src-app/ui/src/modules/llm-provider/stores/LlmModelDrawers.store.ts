import { defineStore } from '@/core/store-kit'

// ===== Add Local Model Upload Drawer =====
export const AddLocalLlmModelUploadDrawer = defineStore('AddLocalLlmModelUploadDrawer', {
  state: { open: false, loading: false, providerId: null as string | null },
  actions: set => ({
    openAddLocalLlmModelUploadDrawer: (providerId: string) => set({ open: true, providerId }),
    closeAddLocalLlmModelUploadDrawer: () =>
      set({ open: false, loading: false, providerId: null }),
    setAddLocalLlmModelUploadDrawerLoading: (loading: boolean) => set({ loading }),
  }),
})
export const useAddLocalLlmModelUploadDrawerStore = AddLocalLlmModelUploadDrawer.store

// ===== Add Local Model Download Drawer =====
export const AddLocalLlmModelDownloadDrawer = defineStore('AddLocalLlmModelDownloadDrawer', {
  state: { open: false, loading: false, providerId: null as string | null },
  actions: set => ({
    openAddLocalLlmModelDownloadDrawer: (providerId: string) => set({ open: true, providerId }),
    closeAddLocalLlmModelDownloadDrawer: () =>
      set({ open: false, loading: false, providerId: null }),
    setAddLocalLlmModelDownloadDrawerLoading: (loading: boolean) => set({ loading }),
  }),
})
export const useAddLocalLlmModelDownloadDrawerStore = AddLocalLlmModelDownloadDrawer.store

// ===== Edit LLM Model Drawer (Unified for Local & Remote) =====
export const EditLlmModelDrawer = defineStore('EditLlmModelDrawer', {
  state: { open: false, loading: false, modelId: null as string | null },
  actions: set => ({
    openEditLlmModelDrawer: (modelId: string) => set({ open: true, modelId }),
    closeEditLlmModelDrawer: () => set({ open: false, loading: false, modelId: null }),
    setEditLlmModelDrawerLoading: (loading: boolean) => set({ loading }),
  }),
  init: ({ on, get, actions }) => {
    on('llm_model.deleted', event => {
      if (get().modelId === event.data.modelId) actions.closeEditLlmModelDrawer()
    })
  },
})
export const useEditLlmModelDrawerStore = EditLlmModelDrawer.store

// ===== Add Remote LLM Model Drawer =====
export const AddRemoteLlmModelDrawer = defineStore('AddRemoteLlmModelDrawer', {
  state: {
    open: false,
    loading: false,
    providerId: null as string | null,
    providerType: null as string | null,
  },
  actions: set => ({
    openAddRemoteLlmModelDrawer: (providerId: string, providerType: string) =>
      set({ open: true, providerId, providerType }),
    closeAddRemoteLlmModelDrawer: () =>
      set({ open: false, loading: false, providerId: null, providerType: null }),
    setAddRemoteLlmModelDrawerLoading: (loading: boolean) => set({ loading }),
  }),
})
export const useAddRemoteLlmModelDrawerStore = AddRemoteLlmModelDrawer.store

// ===== View Download Drawer =====
export const ViewDownloadDrawer = defineStore('ViewDownloadDrawer', {
  state: { open: false, loading: false, downloadId: null as string | null },
  actions: set => ({
    openViewDownloadDrawer: (downloadId: string) => set({ open: true, downloadId }),
    closeViewDownloadDrawer: () => set({ open: false, loading: false, downloadId: null }),
    setViewDownloadDrawerLoading: (loading: boolean) => set({ loading }),
  }),
})
export const useViewDownloadDrawerStore = ViewDownloadDrawer.store

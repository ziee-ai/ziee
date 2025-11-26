import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { Stores } from '@/core/stores'

// ===== Add Local Model Upload Drawer =====
interface AddLocalLlmModelUploadDrawerState {
  open: boolean
  loading: boolean
  providerId: string | null

  // Actions
  openAddLocalLlmModelUploadDrawer: (providerId: string) => void
  closeAddLocalLlmModelUploadDrawer: () => void
  setAddLocalLlmModelUploadDrawerLoading: (loading: boolean) => void
}

export const useAddLocalLlmModelUploadDrawerStore =
  create<AddLocalLlmModelUploadDrawerState>(
    (set): AddLocalLlmModelUploadDrawerState => ({
      open: false,
      loading: false,
      providerId: null,

      // Actions
      openAddLocalLlmModelUploadDrawer: (providerId: string) => {
        set({
          open: true,
          providerId,
        })
      },

      closeAddLocalLlmModelUploadDrawer: () => {
        set({
          open: false,
          loading: false,
          providerId: null,
        })
      },

      setAddLocalLlmModelUploadDrawerLoading: (loading: boolean) => {
        set({ loading })
      },
    }),
  )

// ===== Add Local Model Download Drawer =====
interface AddLocalLlmModelDownloadDrawerState {
  open: boolean
  loading: boolean
  providerId: string | null

  // Actions
  openAddLocalLlmModelDownloadDrawer: (providerId: string) => void
  closeAddLocalLlmModelDownloadDrawer: () => void
  setAddLocalLlmModelDownloadDrawerLoading: (loading: boolean) => void
}

export const useAddLocalLlmModelDownloadDrawerStore =
  create<AddLocalLlmModelDownloadDrawerState>(
    (set): AddLocalLlmModelDownloadDrawerState => ({
      open: false,
      loading: false,
      providerId: null,

      // Actions
      openAddLocalLlmModelDownloadDrawer: (providerId: string) => {
        set({
          open: true,
          providerId,
        })
      },

      closeAddLocalLlmModelDownloadDrawer: () => {
        set({
          open: false,
          loading: false,
          providerId: null,
        })
      },

      setAddLocalLlmModelDownloadDrawerLoading: (loading: boolean) => {
        set({ loading })
      },
    }),
  )

// ===== Edit LLM Model Drawer (Unified for Local & Remote) =====
interface EditLlmModelDrawerState {
  open: boolean
  loading: boolean
  modelId: string | null

  // Actions
  openEditLlmModelDrawer: (modelId: string) => void
  closeEditLlmModelDrawer: () => void
  setEditLlmModelDrawerLoading: (loading: boolean) => void

  // Initialization
  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useEditLlmModelDrawerStore = create<EditLlmModelDrawerState>()(
  subscribeWithSelector(
    (set, get): EditLlmModelDrawerState => ({
      open: false,
      loading: false,
      modelId: null,

      __init__: {
        __store__: () => {
          const GROUP = 'EditLlmModelDrawerStore'
          const eventBus = Stores.EventBus

          // Subscribe to llm_model.deleted
          eventBus.on(
            'llm_model.deleted',
            async event => {
              const { modelId } = event.data
              const state = get()

              if (state.modelId === modelId) {
                get().closeEditLlmModelDrawer()
              }
            },
            GROUP,
          )
        },
      },

      // Actions
      openEditLlmModelDrawer: (modelId: string) => {
        set({
          open: true,
          modelId,
        })
      },

      closeEditLlmModelDrawer: () => {
        set({
          open: false,
          loading: false,
          modelId: null,
        })
      },

      setEditLlmModelDrawerLoading: (loading: boolean) => {
        set({ loading })
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('EditLlmModelDrawerStore')
      },
    }),
  ),
)

// ===== Add Remote LLM Model Drawer =====
interface AddRemoteLlmModelDrawerState {
  open: boolean
  loading: boolean
  providerId: string | null
  providerType: string | null

  // Actions
  openAddRemoteLlmModelDrawer: (
    providerId: string,
    providerType: string,
  ) => void
  closeAddRemoteLlmModelDrawer: () => void
  setAddRemoteLlmModelDrawerLoading: (loading: boolean) => void
}

export const useAddRemoteLlmModelDrawerStore =
  create<AddRemoteLlmModelDrawerState>(
    (set): AddRemoteLlmModelDrawerState => ({
      open: false,
      loading: false,
      providerId: null,
      providerType: null,

      // Actions
      openAddRemoteLlmModelDrawer: (
        providerId: string,
        providerType: string,
      ) => {
        set({
          open: true,
          providerId,
          providerType,
        })
      },

      closeAddRemoteLlmModelDrawer: () => {
        set({
          open: false,
          loading: false,
          providerId: null,
          providerType: null,
        })
      },

      setAddRemoteLlmModelDrawerLoading: (loading: boolean) => {
        set({ loading })
      },
    }),
  )

// ===== View Download Drawer =====
interface ViewDownloadDrawerState {
  open: boolean
  loading: boolean
  downloadId: string | null

  // Actions
  openViewDownloadDrawer: (downloadId: string) => void
  closeViewDownloadDrawer: () => void
  setViewDownloadDrawerLoading: (loading: boolean) => void
}

export const useViewDownloadDrawerStore = create<ViewDownloadDrawerState>(
  (set): ViewDownloadDrawerState => ({
    open: false,
    loading: false,
    downloadId: null,

    // Actions
    openViewDownloadDrawer: (downloadId: string) => {
      set({
        open: true,
        downloadId,
      })
    },

    closeViewDownloadDrawer: () => {
      set({
        open: false,
        loading: false,
        downloadId: null,
      })
    },

    setViewDownloadDrawerLoading: (loading: boolean) => {
      set({ loading })
    },
  }),
)

import { ApiClient } from '@/api-client'
import { Permissions, type VoiceModelStatus } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * Readiness of the configured whisper ggml model on disk + its download.
 *
 * NOTE: the backend model-download endpoint is synchronous (returns the final
 * `VoiceModelStatus`), so there is no SSE progress stream — the UI shows an
 * indeterminate "Downloading…" state on the button while the request is in
 * flight rather than a byte-progress bar.
 */
export const VoiceModel = defineStore('VoiceModel', {
  state: {
    status: null as VoiceModelStatus | null,
    loading: false,
    downloading: false,
    error: null as string | null,
  },
  actions: set => ({
    loadStatus: async () => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
      set({ loading: true, error: null })
      try {
        const status = await ApiClient.Voice.getModelStatus()
        set({ status, loading: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load model status',
          loading: false,
        })
      }
    },
    downloadModel: async (model?: string) => {
      set({ downloading: true, error: null })
      try {
        const status = await ApiClient.Voice.downloadModel(model ? { model } : {})
        set({ status, downloading: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to download model',
          downloading: false,
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, actions }) => {
    // The selected model lives in voice_settings — a settings change (local or
    // remote) can flip which model must be present, so re-check on that sync.
    const reload = () => void actions.loadStatus()
    on('sync:voice_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadStatus()
  },
})

export const useVoiceModelStore = VoiceModel.store

import { defineStore } from '@ziee/framework/store-kit'

/**
 * Open-state for the whisper-model upload drawer. Mirrors the llm-provider
 * `AddLocalLlmModelUploadDrawer` open-store pattern.
 */
export const VoiceUploadModelDrawer = defineStore('VoiceUploadModelDrawer', {
  state: { open: false },
  actions: set => ({
    openUploadModelDrawer: () => set({ open: true }),
    closeUploadModelDrawer: () => set({ open: false }),
  }),
})

export const useVoiceUploadModelDrawerStore = VoiceUploadModelDrawer.store

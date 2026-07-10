import { ApiClient } from '@/api-client'
import { Permissions, type AvailableUpdatesResponse2 } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * Upstream whisper release feed diffed against installed versions. Mirrors
 * llm-local-runtime's `RuntimeUpdate` but single-engine (no engine map).
 */
export const VoiceUpdate = defineStore('VoiceUpdate', {
  state: {
    updateCheck: null as AvailableUpdatesResponse2 | null,
    checking: false,
    error: null as string | null,
  },
  actions: set => ({
    checkForUpdates: async (): Promise<AvailableUpdatesResponse2 | null> => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return null
      set({ checking: true, error: null })
      try {
        const response = await ApiClient.Voice.checkVersionUpdates()
        set({ updateCheck: response, checking: false })
        return response
      } catch (error) {
        set({
          checking: false,
          error: error instanceof Error ? error.message : 'Failed to check updates',
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
})

export const useVoiceUpdateStore = VoiceUpdate.store

import { ApiClient } from '@/api-client'
import {
  Permissions,
  type UpdateVoiceSettingsRequest,
  type VoiceSettings,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Deployment-wide voice settings singleton. Mirrors llm-local-runtime's
 * `RuntimeConfig` (getSettings / updateSettings + sync reload).
 */
export const VoiceConfig = defineStore('VoiceConfig', {
  state: {
    settings: null as VoiceSettings | null,
    loadingSettings: false,
    savingSettings: false,
    error: null as string | null,
  },
  actions: set => ({
    loadSettings: async () => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
      set({ loadingSettings: true, error: null })
      try {
        const settings = await ApiClient.Voice.getSettings()
        set({ settings, loadingSettings: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load voice settings',
          loadingSettings: false,
        })
      }
    },
    saveSettings: async (req: UpdateVoiceSettingsRequest) => {
      set({ savingSettings: true, error: null })
      try {
        const settings = await ApiClient.Voice.updateSettings(req)
        set({ settings, savingSettings: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to save voice settings',
          savingSettings: false,
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadSettings()
    on('sync:voice_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadSettings()
  },
})

export const useVoiceConfigStore = VoiceConfig.store

import { ApiClient } from '@/api-client'
import { Permissions, type VoiceInstanceInfo } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * The single managed whisper-server instance (status/state + restart/stop).
 */
export const VoiceInstance = defineStore('VoiceInstance', {
  state: {
    info: null as VoiceInstanceInfo | null,
    loading: false,
    busy: false,
    error: null as string | null,
  },
  actions: set => ({
    loadInstance: async () => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
      set({ loading: true, error: null })
      try {
        const info = await ApiClient.Voice.getInstance()
        set({ info, loading: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load instance',
          loading: false,
        })
      }
    },
    restartInstance: async () => {
      set({ busy: true, error: null })
      try {
        const info = await ApiClient.Voice.restartInstance()
        set({ info, busy: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to restart instance',
          busy: false,
        })
        throw error
      }
    },
    stopInstance: async () => {
      set({ busy: true, error: null })
      try {
        const info = await ApiClient.Voice.stopInstance()
        set({ info, busy: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to stop instance',
          busy: false,
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadInstance()
    on('sync:reconnect', reload)
    void actions.loadInstance()
  },
})

export const useVoiceInstanceStore = VoiceInstance.store

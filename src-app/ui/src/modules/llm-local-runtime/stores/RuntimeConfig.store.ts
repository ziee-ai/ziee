import { ApiClient } from '@/api-client'
import {
  type GpuDetectionResponse,
  Permissions,
  type RuntimeSettings,
  type UpdateRuntimeSettingsRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

export const RuntimeConfig = defineStore('RuntimeConfig', {
  state: {
    // Singleton runtime settings (idle / auto-start / drain / allow_unsigned)
    settings: null as RuntimeSettings | null,
    loadingSettings: false,
    savingSettings: false,
    // GPU detection result (powers the GPU card)
    gpu: null as GpuDetectionResponse | null,
    loadingGpu: false,
    error: null as string | null,
  },
  actions: set => ({
    loadSettings: async () => {
      if (!hasPermissionNow(Permissions.RuntimeSettingsRead)) return
      set({ loadingSettings: true, error: null })
      try {
        const settings = await ApiClient.LocalRuntime.getRuntimeSettings(undefined)
        set({ settings, loadingSettings: false })
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to load runtime settings',
          loadingSettings: false,
        })
      }
    },
    saveSettings: async (req: UpdateRuntimeSettingsRequest) => {
      set({ savingSettings: true, error: null })
      try {
        const settings = await ApiClient.LocalRuntime.updateRuntimeSettings(req)
        set({ settings, savingSettings: false })
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to save runtime settings',
          savingSettings: false,
        })
        throw error
      }
    },
    loadGpu: async () => {
      set({ loadingGpu: true })
      // detect-gpu spawns host probes and can transiently 502 on a cold backend;
      // retry a few times with backoff before giving up so the card isn't blank.
      const delays = [1000, 2000, 3000]
      for (let attempt = 0; attempt <= delays.length; attempt++) {
        try {
          const gpu = await ApiClient.LocalRuntime.detectGpu(undefined)
          set({ gpu, loadingGpu: false })
          return
        } catch (error) {
          if (attempt === delays.length) {
            set({
              error: error instanceof Error ? error.message : 'GPU detection failed',
              loadingGpu: false,
            })
          } else {
            await new Promise(r => setTimeout(r, delays[attempt]))
          }
        }
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, actions }) => {
    // Cross-device sync: reload the deployment-wide runtime settings (singleton)
    // on a remote change or after an SSE reconnect. loadSettings self-gates.
    const reload = () => void actions.loadSettings()
    on('sync:runtime_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadSettings()
    void actions.loadGpu()
  },
})

export const useRuntimeConfigStore = RuntimeConfig.store

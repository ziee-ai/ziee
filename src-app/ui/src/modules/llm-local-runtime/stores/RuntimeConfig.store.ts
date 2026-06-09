import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  GpuDetectionResponse,
  RuntimeSettings,
  UpdateRuntimeSettingsRequest,
} from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface RuntimeConfigState {
  // Singleton runtime settings (idle / auto-start / drain / allow_unsigned)
  settings: RuntimeSettings | null
  loadingSettings: boolean
  savingSettings: boolean

  // GPU detection result (powers the GPU card)
  gpu: GpuDetectionResponse | null
  loadingGpu: boolean

  error: string | null

  loadSettings: () => Promise<void>
  saveSettings: (req: UpdateRuntimeSettingsRequest) => Promise<void>
  loadGpu: () => Promise<void>
  clearError: () => void

  __init__: {
    __store__: () => void
    settings: () => Promise<void>
    gpu: () => Promise<void>
  }
  __destroy__: () => void
}

export const useRuntimeConfigStore = create<RuntimeConfigState>()(
  subscribeWithSelector((set, get) => ({
    settings: null,
    loadingSettings: false,
    savingSettings: false,
    gpu: null,
    loadingGpu: false,
    error: null,

    loadSettings: async () => {
      if (!hasPermissionNow(Permissions.RuntimeSettingsRead)) return
      set({ loadingSettings: true, error: null })
      try {
        const settings =
          await ApiClient.LocalRuntime.getRuntimeSettings(undefined)
        set({ settings, loadingSettings: false })
      } catch (error) {
        set({
          error:
            error instanceof Error
              ? error.message
              : 'Failed to load runtime settings',
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
            error instanceof Error
              ? error.message
              : 'Failed to save runtime settings',
          savingSettings: false,
        })
        throw error
      }
    },

    loadGpu: async () => {
      set({ loadingGpu: true })
      // detect-gpu spawns host probes and can transiently 502 on a cold
      // backend; retry a few times with backoff before giving up so the card
      // isn't left blank.
      const delays = [1000, 2000, 3000]
      for (let attempt = 0; attempt <= delays.length; attempt++) {
        try {
          const gpu = await ApiClient.LocalRuntime.detectGpu(undefined)
          set({ gpu, loadingGpu: false })
          return
        } catch (error) {
          if (attempt === delays.length) {
            set({
              error:
                error instanceof Error ? error.message : 'GPU detection failed',
              loadingGpu: false,
            })
          } else {
            await new Promise(r => setTimeout(r, delays[attempt]))
          }
        }
      }
    },

    clearError: () => set({ error: null }),

    __init__: {
      __store__: () => {
        const eventBus = Stores.EventBus

        // Cross-device sync: reload the deployment-wide runtime settings
        // (singleton; sync id is nil) on a remote change, or after an SSE
        // reconnect. loadSettings self-gates on RuntimeSettingsRead.
        const reload = () => void get().loadSettings()
        eventBus.on('sync:runtime_settings', reload, 'RuntimeConfigStore')
        eventBus.on('sync:reconnect', reload, 'RuntimeConfigStore')
      },
      settings: () => get().loadSettings(),
      gpu: () => get().loadGpu(),
    },

    __destroy__: () => {
      Stores.EventBus.removeGroupListeners('RuntimeConfigStore')
    },
  })),
)

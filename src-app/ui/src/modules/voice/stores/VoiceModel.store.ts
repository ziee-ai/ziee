import { ApiClient } from '@/api-client'
import {
  Permissions,
  type VoiceModel as VoiceModelRow,
  type VoiceModelStatus,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'

/**
 * The installed whisper-model library + the configured-model readiness status.
 *
 * `installed` is the set of `voice_models` rows (set-active / delete). The
 * downloadable catalog + download progress live in the sibling
 * `VoiceModelUpdate` / `VoiceModelDownloadProgress` stores.
 */
export const VoiceModel = defineStore('VoiceModel', {
  state: {
    status: null as VoiceModelStatus | null,
    installed: [] as VoiceModelRow[],
    loading: false,
    loadingInstalled: false,
    activating: new Map<string, boolean>(),
    deleting: new Map<string, boolean>(),
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
          error:
            error instanceof Error
              ? error.message
              : 'Failed to load model status',
          loading: false,
        })
      }
    },
    loadInstalled: async () => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
      set({ loadingInstalled: true, error: null })
      try {
        const installed = await ApiClient.Voice.listModels()
        set({ installed, loadingInstalled: false })
      } catch (error) {
        set({
          error:
            error instanceof Error
              ? error.message
              : 'Failed to load installed models',
          loadingInstalled: false,
        })
      }
    },
    // Convenience: the downloadable catalog is owned by VoiceModelUpdate (mirrors
    // VoiceUpdate owning the runtime feed). Delegates so callers of a "load
    // catalog" API get one source of truth instead of a duplicated list.
    loadCatalog: async () => {
      await Stores.VoiceModelUpdate.checkForUpdates().catch(() => {
        /* non-fatal */
      })
    },
    activate: async (id: string) => {
      set(state => ({
        activating: new Map(state.activating).set(id, true),
        error: null,
      }))
      try {
        const updated = await ApiClient.Voice.activateModel({ id })
        set(state => {
          const next = new Map(state.activating)
          next.delete(id)
          return {
            activating: next,
            installed: state.installed.map(m => ({
              ...m,
              is_active: m.id === updated.id,
            })),
          }
        })
      } catch (error) {
        set(state => {
          const next = new Map(state.activating)
          next.delete(id)
          return {
            activating: next,
            error:
              error instanceof Error
                ? error.message
                : 'Failed to activate model',
          }
        })
        throw error
      }
    },
    remove: async (id: string, ackActive = false) => {
      set(state => ({
        deleting: new Map(state.deleting).set(id, true),
        error: null,
      }))
      try {
        await ApiClient.Voice.deleteModel({ id, ack_active: ackActive })
        set(state => {
          const next = new Map(state.deleting)
          next.delete(id)
          return {
            deleting: next,
            installed: state.installed.filter(m => m.id !== id),
          }
        })
      } catch (error) {
        set(state => {
          const next = new Map(state.deleting)
          next.delete(id)
          return {
            deleting: next,
            error:
              error instanceof Error ? error.message : 'Failed to delete model',
          }
        })
        throw error
      }
    },
    clearError: () => set({ error: null }),
  }),
  init: ({ on, actions }) => {
    // A model add/activate/delete (local or remote) invalidates the installed
    // set; the selected model lives in voice_settings, so a settings change can
    // flip readiness. Both refetches self-gate on VoiceAdminRead (no-403 rule).
    const reloadModels = () => void actions.loadInstalled()
    const reloadStatus = () => void actions.loadStatus()
    on('sync:voice_model', reloadModels)
    on('sync:voice_settings', reloadStatus)
    on('sync:reconnect', () => {
      reloadModels()
      reloadStatus()
    })
    void actions.loadInstalled()
    void actions.loadStatus()
  },
})

export const useVoiceModelStore = VoiceModel.store

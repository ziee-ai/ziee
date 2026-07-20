import { ApiClient } from '@/api-client'
import { type RuntimeVersionResponse2 } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'

/**
 * Installed whisper runtime versions. Mirrors the llm-local-runtime
 * `RuntimeVersion` store but single-engine (whisper) — no engine dimension.
 */
export const VoiceRuntimeVersion = defineStore('VoiceRuntimeVersion', {
  state: {
    versions: [] as RuntimeVersionResponse2[],
    isInitialized: false,
    loading: false,
    settingDefault: new Map<string, boolean>(),
    deleting: new Map<string, boolean>(),
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadVersions = async () => {
      if (!hasPermissionNow(Permissions.VoiceAdminRead)) return
      set({ loading: true, error: null })
      try {
        const response = await ApiClient.Voice.listVersions({})
        set({
          versions: response.versions || [],
          isInitialized: true,
          loading: false,
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load versions',
          loading: false,
        })
      }
    }
    return {
      loadVersions,
      setDefaultVersion: async (id: string) => {
        set(state => ({
          settingDefault: new Map(state.settingDefault).set(id, true),
          error: null,
        }))
        try {
          await ApiClient.Voice.setDefaultVersion({ id })
          set(state => {
            const versions = state.versions.map(v => ({
              ...v,
              is_system_default: v.id === id,
            }))
            const newMap = new Map(state.settingDefault)
            newMap.delete(id)
            return { versions, settingDefault: newMap }
          })
        } catch (error) {
          set(state => {
            const newMap = new Map(state.settingDefault)
            newMap.delete(id)
            return {
              settingDefault: newMap,
              error: error instanceof Error ? error.message : 'Failed to set default',
            }
          })
          throw error
        }
      },
      deleteVersion: async (id: string, removeBinary = false) => {
        set(state => ({
          deleting: new Map(state.deleting).set(id, true),
          error: null,
        }))
        try {
          await ApiClient.Voice.deleteVersion({ id, remove_binary: removeBinary })
          set(state => {
            const newMap = new Map(state.deleting)
            newMap.delete(id)
            return { versions: state.versions.filter(v => v.id !== id), deleting: newMap }
          })
        } catch (error) {
          set(state => {
            const newMap = new Map(state.deleting)
            newMap.delete(id)
            return {
              deleting: newMap,
              error: error instanceof Error ? error.message : 'Failed to delete version',
            }
          })
          throw error
        }
      },
      syncCache: async () => {
        set({ loading: true, error: null })
        try {
          await ApiClient.Voice.syncVersionCache()
          await loadVersions()
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to sync cache',
            loading: false,
          })
        }
      },
      getDefaultVersion: (): RuntimeVersionResponse2 | null =>
        get().versions.find(v => v.is_system_default) || null,
      clearError: () => set({ error: null }),
    }
  },
  init: ({ on, actions }) => {
    // Cross-device sync: reload on a remote download/delete/default change or an
    // SSE reconnect. loadVersions self-gates on VoiceAdminRead (no-403 rule).
    const reload = () => void actions.loadVersions()
    on('sync:voice_runtime_version', reload)
    on('sync:reconnect', reload)
    void actions.loadVersions()
    // Keep the update-check fresh when the installed set is invalidated.
    on('sync:voice_runtime_version', () => {
      if (hasPermissionNow(Permissions.VoiceAdminRead)) {
        Stores.VoiceUpdate.checkForUpdates().catch(() => {})
      }
    })
  },
})

export const useVoiceRuntimeVersionStore = VoiceRuntimeVersion.store

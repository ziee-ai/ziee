import { ApiClient } from '@/api-client'
import { type DownloadVersionRequest, type RuntimeVersionResponse } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { Stores } from '@ziee/framework/stores'
import {
  emitRuntimeVersionDefaultChanged,
  emitRuntimeVersionDeleted,
} from '../events/emitters'
import type { RuntimeEngine } from '../types'

export const RuntimeVersion = defineStore('RuntimeVersion', {
  state: {
    versions: [] as RuntimeVersionResponse[],
    isInitialized: false,
    loading: false,
    downloading: new Map<string, boolean>(), // version_id -> downloading
    settingDefault: new Map<string, boolean>(),
    deleting: new Map<string, boolean>(),
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadVersions = async (engine?: RuntimeEngine) => {
      if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
      set({ loading: true, error: null })
      try {
        const response = await ApiClient.RuntimeVersion.list({ engine })
        set({ versions: response.versions || [], isInitialized: true, loading: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load versions',
          loading: false,
        })
      }
    }
    return {
      loadVersions,
      // Kick off a detached download. Delegates to the RuntimeDownloadProgress
      // store, which opens the SSE subscription + refreshes `versions` on
      // Complete — so this no longer returns the resulting RuntimeVersion.
      downloadVersion: async (request: DownloadVersionRequest): Promise<{ key: string }> => {
        set({ error: null })
        try {
          return await Stores.RuntimeDownloadProgress.startDownload(request)
        } catch (error) {
          const message = error instanceof Error ? error.message : 'Download failed'
          set({ error: message })
          throw error
        }
      },
      setDefaultVersion: async (versionId: string) => {
        set(state => ({
          settingDefault: new Map(state.settingDefault).set(versionId, true),
          error: null,
        }))
        try {
          await ApiClient.RuntimeVersion.setDefault({ version_id: versionId })
          await emitRuntimeVersionDefaultChanged(versionId)
          set(state => {
            const version = state.versions.find(v => v.id === versionId)
            if (!version) return state
            const updatedVersions = state.versions.map(v => ({
              ...v,
              is_system_default:
                v.engine === version.engine ? v.id === versionId : v.is_system_default,
            }))
            const newMap = new Map(state.settingDefault)
            newMap.delete(versionId)
            return { versions: updatedVersions, settingDefault: newMap }
          })
        } catch (error) {
          set(state => {
            const newMap = new Map(state.settingDefault)
            newMap.delete(versionId)
            return {
              settingDefault: newMap,
              error: error instanceof Error ? error.message : 'Failed to set default',
            }
          })
          throw error
        }
      },
      deleteVersion: async (versionId: string, removeBinary = false) => {
        set(state => ({
          deleting: new Map(state.deleting).set(versionId, true),
          error: null,
        }))
        try {
          await ApiClient.RuntimeVersion.delete({
            version_id: versionId,
            remove_binary: removeBinary,
          })
          await emitRuntimeVersionDeleted(versionId)
          set(state => {
            const newMap = new Map(state.deleting)
            newMap.delete(versionId)
            return { versions: state.versions.filter(v => v.id !== versionId), deleting: newMap }
          })
        } catch (error) {
          set(state => {
            const newMap = new Map(state.deleting)
            newMap.delete(versionId)
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
          await ApiClient.RuntimeVersion.syncCache()
          await loadVersions() // Reload after sync
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to sync cache',
            loading: false,
          })
        }
      },
      getVersionsByEngine: (engine: RuntimeEngine): RuntimeVersionResponse[] =>
        get().versions.filter(v => v.engine === engine),
      getDefaultVersion: (engine: RuntimeEngine): RuntimeVersionResponse | null =>
        get().versions.find(v => v.engine === engine && v.is_system_default) || null,
      clearError: () => set({ error: null }),
    }
  },
  init: ({ on, get, set, actions }) => {
    on('runtime_version.created', event => {
      const state = get()
      if (!state.versions.find(v => v.id === event.data.version.id)) {
        set({ versions: [...state.versions, event.data.version] })
      }
    })
    on('runtime_version.deleted', event => {
      set(state => ({
        versions: state.versions.filter(v => v.id !== event.data.versionId),
      }))
    })
    on('runtime_version.default_changed', event => {
      set(state => {
        const version = state.versions.find(v => v.id === event.data.versionId)
        if (!version) return state
        return {
          versions: state.versions.map(v => ({
            ...v,
            is_system_default:
              v.engine === version.engine ? v.id === event.data.versionId : v.is_system_default,
          })),
        }
      })
    })
    // Cross-device sync: reload on a remote download/delete/default change, or
    // after an SSE reconnect. loadVersions self-gates on RuntimeVersionRead.
    const reload = () => void actions.loadVersions()
    on('sync:runtime_version', reload)
    on('sync:reconnect', reload)
    void actions.loadVersions()
  },
})

export const useRuntimeVersionStore = RuntimeVersion.store

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  DownloadVersionRequest,
  RuntimeVersionResponse,
} from '@/api-client/types'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import {
  emitRuntimeVersionDefaultChanged,
  emitRuntimeVersionDeleted,
} from '../events/emitters'
import type { RuntimeEngine } from '../types'

interface RuntimeVersionState {
  // Data
  versions: RuntimeVersionResponse[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  downloading: Map<string, boolean> // version_id -> downloading
  settingDefault: Map<string, boolean>
  deleting: Map<string, boolean>
  error: string | null

  // Actions
  loadVersions: (engine?: RuntimeEngine) => Promise<void>
  /** Kick off a detached download. Returns the registry key — the
   *  resulting RuntimeVersion lands in `versions` once the
   *  RuntimeDownloadProgress store sees the Complete SSE event and
   *  calls `loadVersions()` for us. */
  downloadVersion: (request: DownloadVersionRequest) => Promise<{ key: string }>
  setDefaultVersion: (versionId: string) => Promise<void>
  deleteVersion: (versionId: string, removeBinary?: boolean) => Promise<void>
  syncCache: () => Promise<void>

  // Selectors
  getVersionsByEngine: (engine: RuntimeEngine) => RuntimeVersionResponse[]
  getDefaultVersion: (engine: RuntimeEngine) => RuntimeVersionResponse | null

  // Error handling
  clearError: () => void

  // Initialization
  __init__: {
    __store__: () => void
    versions: () => Promise<void>
  }
  __destroy__: () => void
}

export const useRuntimeVersionStore = create<RuntimeVersionState>()(
  subscribeWithSelector((set, get) => ({
    versions: [],
    isInitialized: false,
    loading: false,
    downloading: new Map(),
    settingDefault: new Map(),
    deleting: new Map(),
    error: null,

    loadVersions: async (engine?: RuntimeEngine) => {
      if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
      set({ loading: true, error: null })
      try {
        const response = await ApiClient.RuntimeVersion.list({
          engine,
        })

        set({
          versions: response.versions || [],
          isInitialized: true,
          loading: false,
        })
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to load versions',
          loading: false,
        })
      }
    },

    downloadVersion: async (request: DownloadVersionRequest) => {
      // Delegates to the detached-task pipeline owned by the
      // RuntimeDownloadProgress store. That store opens the SSE
      // subscription, drives the per-row progress bar, and refreshes
      // `versions` here on Complete — so this action no longer
      // returns the resulting RuntimeVersion (it doesn't exist yet
      // when the POST returns). Callers that need the version row
      // should watch `Stores.RuntimeVersion.versions` instead.
      set({ error: null })
      try {
        return await Stores.RuntimeDownloadProgress.startDownload(request)
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'Download failed'
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
        await ApiClient.RuntimeVersion.setDefault({
          version_id: versionId,
        })

        await emitRuntimeVersionDefaultChanged(versionId)

        set(state => {
          const version = state.versions.find(v => v.id === versionId)
          if (!version) return state

          // Unset previous default for this engine
          const updatedVersions = state.versions.map(v => ({
            ...v,
            is_system_default:
              v.engine === version.engine
                ? v.id === versionId
                : v.is_system_default,
          }))

          const newMap = new Map(state.settingDefault)
          newMap.delete(versionId)

          return {
            versions: updatedVersions,
            settingDefault: newMap,
          }
        })
      } catch (error) {
        set(state => {
          const newMap = new Map(state.settingDefault)
          newMap.delete(versionId)
          return {
            settingDefault: newMap,
            error:
              error instanceof Error ? error.message : 'Failed to set default',
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

          return {
            versions: state.versions.filter(v => v.id !== versionId),
            deleting: newMap,
          }
        })
      } catch (error) {
        set(state => {
          const newMap = new Map(state.deleting)
          newMap.delete(versionId)
          return {
            deleting: newMap,
            error:
              error instanceof Error
                ? error.message
                : 'Failed to delete version',
          }
        })
        throw error
      }
    },

    syncCache: async () => {
      set({ loading: true, error: null })
      try {
        await ApiClient.RuntimeVersion.syncCache()
        await get().loadVersions() // Reload after sync
      } catch (error) {
        set({
          error:
            error instanceof Error ? error.message : 'Failed to sync cache',
          loading: false,
        })
      }
    },

    getVersionsByEngine: (engine: RuntimeEngine) => {
      return get().versions.filter(v => v.engine === engine)
    },

    getDefaultVersion: (engine: RuntimeEngine) => {
      return (
        get().versions.find(v => v.engine === engine && v.is_system_default) ||
        null
      )
    },

    clearError: () => set({ error: null }),

    __init__: {
      __store__: () => {
        const eventBus = Stores.EventBus

        eventBus.on(
          'runtime_version.created',
          event => {
            const state = get()
            if (!state.versions.find(v => v.id === event.data.version.id)) {
              set({ versions: [...state.versions, event.data.version] })
            }
          },
          'RuntimeVersionStore',
        )

        eventBus.on(
          'runtime_version.deleted',
          event => {
            set(state => ({
              versions: state.versions.filter(
                v => v.id !== event.data.versionId,
              ),
            }))
          },
          'RuntimeVersionStore',
        )

        eventBus.on(
          'runtime_version.default_changed',
          event => {
            set(state => {
              const version = state.versions.find(
                v => v.id === event.data.versionId,
              )
              if (!version) return state

              return {
                versions: state.versions.map(v => ({
                  ...v,
                  is_system_default:
                    v.engine === version.engine
                      ? v.id === event.data.versionId
                      : v.is_system_default,
                })),
              }
            })
          },
          'RuntimeVersionStore',
        )

        // Cross-device sync: reload the version list on a remote
        // download/delete/default change, or after an SSE reconnect.
        // loadVersions self-gates on RuntimeVersionRead.
        const reload = () => void get().loadVersions()
        eventBus.on('sync:runtime_version', reload, 'RuntimeVersionStore')
        eventBus.on('sync:reconnect', reload, 'RuntimeVersionStore')
      },
      versions: () => get().loadVersions(),
    },

    __destroy__: () => {
      Stores.EventBus.removeGroupListeners('RuntimeVersionStore')
    },
  })),
)

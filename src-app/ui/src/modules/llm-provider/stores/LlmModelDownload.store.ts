import { ApiClient } from '@/api-client'
import {
  type DownloadFromRepositoryRequest,
  type DownloadInstance,
  Permissions,
  type RepositoryFileListResponse,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import { useLlmProviderStore } from '@/modules/llm-provider/stores/LlmProvider.store'
import {
  emitLlmModelDownloadCompleted,
  emitLlmModelDownloadFailed,
} from '@/modules/llm-provider/events/emitters'

// SSE abort controller + one-time self-subscribe guard (module-scope: not
// serializable / reactive).
let sseAbortController: AbortController | null = null
let isSubscriptionSetup = false

// Load existing downloads from server.
const loadExistingDownloads = async (set: any): Promise<void> => {
  // Permission-gate the shell-eager-load fetch: the store init fires for every
  // authenticated user; without the gate non-admins 403 on every page render.
  if (!hasPermissionNow(Permissions.LlmModelsDownloadsRead)) return
  try {
    const response = await ApiClient.LlmModel.listDownloads({ page: 1, per_page: 100 })
    // Keep only pending/downloading/failed (exclude completed and cancelled).
    const downloads = (response?.downloads ?? []).filter(download =>
      ['pending', 'downloading', 'failed'].includes(download.status),
    )
    set({ downloads })
  } catch (error) {
    console.error('Failed to load downloads:', error)
  }
}

export const LlmModelDownload = defineStore('LlmModelDownload', {
  state: {
    downloads: [] as DownloadInstance[],
    sseConnected: false,
    sseError: null as string | null,
    reconnectAttempts: 0,
    isInitialized: false,
  },
  actions: (set, get) => {
    const disconnectSSE = (): void => {
      if (sseAbortController) {
        sseAbortController.abort()
        sseAbortController = null
      }
      set({ sseConnected: false, reconnectAttempts: 0 })
    }

    const subscribeToDownloadProgress = async (): Promise<void> => {
      const state = get()
      // Don't reconnect if already connected.
      if (state.sseConnected || sseAbortController) return
      try {
        await ApiClient.LlmModel.subscribeDownloadProgress(undefined, {
          SSE: {
            __init: ({ abortController }) => {
              sseAbortController = abortController
              set({ sseConnected: true, sseError: null, reconnectAttempts: 0 })
            },
            connected: (_data: { message?: string }) => {},
            update: (updates: any[]) => {
              // Snapshot pre-update status by id. The terminal emits must fire
              // EXACTLY ONCE per download — keying off "prev !== current AND
              // current is terminal" guarantees that.
              const prevState = get()
              const prevStatusById = new Map<string, string>(
                prevState.downloads.map(d => [d.id, d.status]),
              )
              // Refresh providers of newly completed downloads.
              const newlyCompleted = updates.filter((u: any) => u.status === 'completed')
              if (newlyCompleted.length > 0) {
                const providerIds = [
                  ...new Set(
                    newlyCompleted
                      .map((d: any) => d.provider_id)
                      .filter((id: string | undefined): id is string => !!id),
                  ),
                ]
                for (const providerId of providerIds) {
                  void useLlmProviderStore.getState().loadModelsForProvider(providerId)
                }
              }
              // Emit terminal-transition events for rows that FLIPPED this tick.
              for (const u of updates as any[]) {
                if (!u.id || typeof u.status !== 'string') continue
                const prev = prevStatusById.get(u.id)
                if (prev === u.status) continue
                const isNewlyTerminal =
                  (u.status === 'completed' || u.status === 'failed') && prev !== undefined
                if (!isNewlyTerminal) continue
                const priorRow = prevState.downloads.find(d => d.id === u.id)
                const displayName =
                  priorRow?.request_data?.display_name ||
                  priorRow?.request_data?.model_name ||
                  'Model'
                if (u.status === 'completed') {
                  void emitLlmModelDownloadCompleted(
                    u.id,
                    u.provider_id ?? priorRow?.provider_id ?? '',
                    displayName,
                  )
                } else {
                  void emitLlmModelDownloadFailed(
                    u.id,
                    u.provider_id ?? priorRow?.provider_id ?? '',
                    displayName,
                    u.error_message ?? priorRow?.error_message ?? '',
                  )
                }
              }
              set(state => {
                const updatedDownloads = state.downloads.map(download => {
                  const update = updates.find((u: any) => u.id === download.id)
                  return update ? { ...download, ...update } : download
                })
                // Drop cancelled + completed. Failed rows stay so the card can
                // render the failure + Retry until dismissed/retried.
                const filteredDownloads = updatedDownloads.filter(
                  download => download.status !== 'cancelled' && download.status !== 'completed',
                )
                return { downloads: filteredDownloads }
              })
            },
            complete: (_data: string) => {
              // Provider IDs from all downloads before they're filtered out.
              const allDownloads = get().downloads
              const providerIds = [
                ...new Set(allDownloads.map(d => d.provider_id).filter((id): id is string => !!id)),
              ]
              for (const providerId of providerIds) {
                void useLlmProviderStore.getState().loadModelsForProvider(providerId)
              }
              disconnectSSE()
              void loadExistingDownloads(set)
            },
            error: (errorMessage: string) => {
              console.error('SSE error:', errorMessage)
              set({ sseError: errorMessage, sseConnected: false })
            },
            default: (event, data) => {
              console.warn('Unknown SSE event:', event, data)
            },
          },
        })
      } catch (error) {
        console.error('SSE connection failed:', error)
        const attempts = get().reconnectAttempts + 1
        const maxAttempts = 5
        if (attempts < maxAttempts) {
          set({
            sseConnected: false,
            sseError: 'Connection lost, reconnecting...',
            reconnectAttempts: attempts,
          })
          setTimeout(() => {
            void subscribeToDownloadProgress()
          }, 3000)
        } else {
          console.error('Max reconnection attempts reached')
          set({
            sseConnected: false,
            sseError: 'Failed to connect to download updates',
            reconnectAttempts: attempts,
          })
        }
      }
    }

    // Subscribe to store changes to manage the SSE connection. fireImmediately
    // runs the callback with current state on setup.
    const setupDownloadTracking = (): void => {
      if (isSubscriptionSetup) return
      isSubscriptionSetup = true
      useLlmModelDownloadStore.subscribe(
        state => state.downloads,
        downloads => {
          const activeDownloads = downloads.filter(
            d => d.status === 'downloading' || d.status === 'pending',
          )
          const state = get()
          if (activeDownloads.length > 0 && !state.sseConnected) {
            void subscribeToDownloadProgress()
          } else if (activeDownloads.length === 0 && state.sseConnected) {
            disconnectSSE()
          }
        },
        { fireImmediately: true },
      )
    }

    const initializeDownloadTracking = async (): Promise<void> => {
      if (get().isInitialized) return
      try {
        await loadExistingDownloads(set)
        setupDownloadTracking()
        set({ isInitialized: true })
      } catch (error) {
        console.error('Failed to initialize download tracking:', error)
      }
    }

    return {
      subscribeToDownloadProgress,
      disconnectSSE,
      setupDownloadTracking,
      initializeDownloadTracking,
      downloadLlmModelFromRepository: async (
        request: DownloadFromRepositoryRequest,
        onStart?: (downloadId: string) => void,
      ): Promise<{ downloadId: string }> => {
        try {
          const downloadInstance = await ApiClient.LlmModel.download(request)
          set(state => ({ downloads: [...state.downloads, downloadInstance] }))
          onStart?.(downloadInstance.id)
          setupDownloadTracking()
          return { downloadId: downloadInstance.id }
        } catch (error) {
          console.error('Failed to initiate download:', error)
          throw error
        }
      },
      // Detect model files at a repo path so the form can offer a picker.
      listRepositoryFiles: async (
        repositoryId: string,
        path: string,
        branch?: string,
      ): Promise<RepositoryFileListResponse> => {
        return await ApiClient.LlmModel.listRepositoryFiles({
          repository_id: repositoryId,
          path,
          branch: branch || 'main',
        })
      },
      addExternalDownload: (instance: DownloadInstance): void => {
        set(state => ({ downloads: [...state.downloads, instance] }))
        setupDownloadTracking()
      },
      cancelLlmModelDownload: async (downloadId: string): Promise<void> => {
        try {
          await ApiClient.LlmModel.cancelDownload({ download_id: downloadId })
          set(state => ({
            downloads: state.downloads.filter(download => download.id !== downloadId),
          }))
        } catch (error) {
          console.error('Failed to cancel download:', error)
          throw error
        }
      },
      deleteLlmModelDownload: async (downloadId: string): Promise<void> => {
        try {
          await ApiClient.LlmModel.deleteDownload({ download_id: downloadId })
          set(state => ({
            downloads: state.downloads.filter(download => download.id !== downloadId),
          }))
        } catch (error) {
          console.error('Failed to delete download:', error)
          throw error
        }
      },
      clearLlmModelDownload: (downloadId: string): void => {
        set(state => ({
          downloads: state.downloads.filter(download => download.id !== downloadId),
        }))
      },
      clearAllLlmModelDownloads: (): void => {
        set({ downloads: [] })
      },
      getAllActiveDownloads: (): DownloadInstance[] =>
        get().downloads.filter(
          download => download.status === 'downloading' || download.status === 'pending',
        ),
      findDownloadById: (downloadId: string): DownloadInstance | undefined =>
        get().downloads.find(download => download.id === downloadId),
    }
  },
  init: ({ actions, onCleanup }) => {
    void actions.initializeDownloadTracking()
    // Abort the module-scope SSE controller on store destroy. (audit 09 B-8)
    onCleanup(() => {
      if (sseAbortController) {
        sseAbortController.abort()
        sseAbortController = null
      }
    })
  },
})

export const useLlmModelDownloadStore = LlmModelDownload.store

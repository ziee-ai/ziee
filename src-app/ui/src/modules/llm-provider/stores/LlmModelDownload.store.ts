import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import {
  Permissions,
  type DownloadFromRepositoryRequest,
  type DownloadInstance,
  type RepositoryFileListResponse,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { useLlmProviderStore } from '@/modules/llm-provider/stores/LlmProvider.store'
import {
  emitLlmModelDownloadCompleted,
  emitLlmModelDownloadFailed,
} from '@/modules/llm-provider/events/emitters'

interface LlmModelDownloadState {
  // Download instances array
  downloads: DownloadInstance[]
  // SSE connection state
  sseConnected: boolean
  sseError: string | null
  // Reconnection attempt count
  reconnectAttempts: number
  // Initialization state
  isInitialized: boolean

  __init__: {
    downloads: () => Promise<void>
  }
  __destroy__?: () => void

  // Actions
  downloadLlmModelFromRepository: (
    request: DownloadFromRepositoryRequest,
    onStart?: (downloadId: string) => void,
  ) => Promise<{ downloadId: string }>
  listRepositoryFiles: (
    repositoryId: string,
    path: string,
    branch?: string,
  ) => Promise<RepositoryFileListResponse>
  addExternalDownload: (instance: DownloadInstance) => void
  cancelLlmModelDownload: (downloadId: string) => Promise<void>
  deleteLlmModelDownload: (downloadId: string) => Promise<void>
  clearLlmModelDownload: (downloadId: string) => void
  clearAllLlmModelDownloads: () => void
  getAllActiveDownloads: () => DownloadInstance[]
  findDownloadById: (downloadId: string) => DownloadInstance | undefined
  subscribeToDownloadProgress: () => Promise<void>
  disconnectSSE: () => void
  setupDownloadTracking: () => void
  initializeDownloadTracking: () => Promise<void>
}

// SSE abort controller for connection management
let sseAbortController: AbortController | null = null
let isSubscriptionSetup = false

// Load existing downloads from server
const loadExistingDownloads = async (set: any): Promise<void> => {
  // Permission-gate the shell-eager-load fetch (audit follow-up):
  // the downloads section is mounted as part of the LLM-providers
  // admin surface but the store __init__ fires for every authenticated
  // user. Without the gate, non-admin users 403 on every page render.
  if (!hasPermissionNow(Permissions.LlmModelsDownloadsRead)) return

  try {
    const response = await ApiClient.LlmModel.listDownloads({
      page: 1,
      per_page: 100,
    })

    // Filter to only keep pending, downloading, and failed
    // (exclude completed and cancelled)
    const downloads = response.downloads.filter(download =>
      ['pending', 'downloading', 'failed'].includes(download.status),
    )

    set({
      downloads,
    })
  } catch (error) {
    console.error('Failed to load downloads:', error)
  }
}

export const useLlmModelDownloadStore = create<LlmModelDownloadState>()(
  subscribeWithSelector(
    (set, get): LlmModelDownloadState => ({
      // Initial state
      downloads: [],
      sseConnected: false,
      sseError: null,
      reconnectAttempts: 0,
      isInitialized: false,
      __init__: {
        downloads: async () => {
          await get().initializeDownloadTracking()
        },
      },

      // Actions
      downloadLlmModelFromRepository: async (
        request: DownloadFromRepositoryRequest,
        onStart?: (downloadId: string) => void,
      ): Promise<{ downloadId: string }> => {
        try {
          // Call the new initiate download endpoint that returns immediately
          const downloadInstance = await ApiClient.LlmModel.download(request)

          // Add to downloads array
          set(state => ({
            downloads: [...state.downloads, downloadInstance],
          }))

          // Call onStart callback with the download ID
          onStart?.(downloadInstance.id)

          // Set up download tracking subscription if not already done
          get().setupDownloadTracking()

          return { downloadId: downloadInstance.id }
        } catch (error) {
          console.error('Failed to initiate download:', error)
          throw error
        }
      },

      // Detect the model files available at a repository path (Hugging Face /
      // GitHub) so the download form can offer a picker instead of a
      // hand-typed filename. Stateless pass-through to the backend detector.
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
        set(state => ({
          downloads: [...state.downloads, instance],
        }))
        get().setupDownloadTracking()
      },

      cancelLlmModelDownload: async (downloadId: string): Promise<void> => {
        try {
          // Call backend to cancel download
          await ApiClient.LlmModel.cancelDownload({ download_id: downloadId })

          // Remove from local state immediately (backend will send update via SSE)
          set(state => ({
            downloads: state.downloads.filter(
              download => download.id !== downloadId,
            ),
          }))
        } catch (error) {
          console.error('Failed to cancel download:', error)
          throw error
        }
      },

      deleteLlmModelDownload: async (downloadId: string): Promise<void> => {
        try {
          // Call backend to delete download
          await ApiClient.LlmModel.deleteDownload({ download_id: downloadId })

          // Remove from local state
          set(state => ({
            downloads: state.downloads.filter(
              download => download.id !== downloadId,
            ),
          }))
        } catch (error) {
          console.error('Failed to delete download:', error)
          throw error
        }
      },

      clearLlmModelDownload: (downloadId: string): void => {
        set(state => ({
          downloads: state.downloads.filter(
            download => download.id !== downloadId,
          ),
        }))
      },

      clearAllLlmModelDownloads: (): void => {
        set({ downloads: [] })
      },

      getAllActiveDownloads: (): DownloadInstance[] => {
        const state = get()
        return state.downloads.filter(
          download =>
            download.status === 'downloading' || download.status === 'pending',
        )
      },

      findDownloadById: (downloadId: string): DownloadInstance | undefined => {
        return get().downloads.find(download => download.id === downloadId)
      },

      subscribeToDownloadProgress: async (): Promise<void> => {
        const state = get()

        // Don't reconnect if already connected
        if (state.sseConnected || sseAbortController) {
          return
        }


        try {
          // Call ApiClient with SSE handlers
          await ApiClient.LlmModel.subscribeDownloadProgress(undefined, {
            SSE: {
              __init: ({ abortController }) => {
                // Store abort controller for manual disconnection
                sseAbortController = abortController
                set({
                  sseConnected: true,
                  sseError: null,
                  reconnectAttempts: 0,
                })
              },

              connected: (_data: { message?: string }) => {},

              update: (updates: any[]) => {

                // Snapshot pre-update status by id. The EventBus emits
                // for `llm_model.download_completed` /
                // `llm_model.download_failed` must fire EXACTLY ONCE
                // per download — keying off "prev !== current AND
                // current is terminal" guarantees that. A row that
                // re-appears on a subsequent SSE tick with the same
                // status won't re-emit; a row appearing for the first
                // time with no prev status doesn't emit either (only
                // explicit transitions count).
                const prevState = get()
                const prevStatusById = new Map<string, string>(
                  prevState.downloads.map(d => [d.id, d.status]),
                )

                // Detect newly completed downloads and refresh their providers' models
                const newlyCompleted = updates.filter(
                  (u: any) => u.status === 'completed',
                )
                if (newlyCompleted.length > 0) {
                  // Extract unique provider IDs from completed downloads
                  const providerIds = [
                    ...new Set(
                      newlyCompleted
                        .map((d: any) => d.provider_id)
                        .filter((id: string | undefined): id is string => !!id),
                    ),
                  ]

                  // Refresh models for each provider
                  for (const providerId of providerIds) {
                    void useLlmProviderStore
                      .getState()
                      .loadModelsForProvider(providerId)
                  }
                }

                // Emit terminal-transition events for any row that
                // FLIPPED to completed/failed THIS tick. Pulls
                // display_name from the prior store row's
                // `request_data.display_name` (or model_name fallback)
                // since the SSE update payload doesn't always include
                // request_data.
                for (const u of updates as any[]) {
                  if (!u.id || typeof u.status !== 'string') continue
                  const prev = prevStatusById.get(u.id)
                  if (prev === u.status) continue
                  const isNewlyTerminal =
                    (u.status === 'completed' || u.status === 'failed') &&
                    // Only emit when we have a prior status — i.e. the
                    // row was being tracked. A first-sighting terminal
                    // status (rare; rows are added at 'pending') would
                    // otherwise risk a duplicate toast on page reload
                    // if the backend ever started replaying terminal
                    // rows on subscribe.
                    prev !== undefined
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
                    const update = updates.find(
                      (u: any) => u.id === download.id,
                    )
                    return update ? { ...download, ...update } : download
                  })

                  // Filter out cancelled and completed downloads before updating state.
                  // Failed rows stay in the array so the hub card (and widget popover)
                  // can render the failure state + Retry button until the user
                  // dismisses or retries.
                  const filteredDownloads = updatedDownloads.filter(
                    download =>
                      download.status !== 'cancelled' &&
                      download.status !== 'completed',
                  )

                  return { downloads: filteredDownloads }
                })
              },

              complete: (_data: string) => {

                // Get provider IDs from all downloads before they're filtered out
                const allDownloads = get().downloads
                const providerIds = [
                  ...new Set(
                    allDownloads
                      .map(d => d.provider_id)
                      .filter((id): id is string => !!id),
                  ),
                ]

                // Refresh models for all providers that had downloads
                for (const providerId of providerIds) {
                  void useLlmProviderStore
                    .getState()
                    .loadModelsForProvider(providerId)
                }

                // Disconnect and reload downloads
                get().disconnectSSE()
                void loadExistingDownloads(set)
              },

              error: (errorMessage: string) => {
                console.error('SSE error:', errorMessage)
                set({
                  sseError: errorMessage,
                  sseConnected: false,
                })
              },

              default: (event, data) => {
                console.warn('Unknown SSE event:', event, data)
              },
            },
          })
        } catch (error) {
          console.error('SSE connection failed:', error)

          const state = get()
          const attempts = state.reconnectAttempts + 1
          const maxAttempts = 5

          if (attempts < maxAttempts) {
            set({
              sseConnected: false,
              sseError: 'Connection lost, reconnecting...',
              reconnectAttempts: attempts,
            })

            // Retry after 3 seconds
            setTimeout(() => {
              void get().subscribeToDownloadProgress()
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
      },

      disconnectSSE: (): void => {

        if (sseAbortController) {
          sseAbortController.abort()
          sseAbortController = null
        }

        set({
          sseConnected: false,
          reconnectAttempts: 0,
        })
      },

      // Set up download tracking subscription
      setupDownloadTracking: (): void => {
        if (isSubscriptionSetup) return
        isSubscriptionSetup = true

        // Subscribe to store changes to manage SSE connection
        // fireImmediately: true ensures the callback runs with current state on setup
        useLlmModelDownloadStore.subscribe(
          state => state.downloads,
          downloads => {
            const activeDownloads = downloads.filter(
              d => d.status === 'downloading' || d.status === 'pending',
            )

            const state = get()

            if (activeDownloads.length > 0 && !state.sseConnected) {
              // We have active downloads but no SSE connection, establish one
              void get().subscribeToDownloadProgress()
            } else if (activeDownloads.length === 0 && state.sseConnected) {
              // No active downloads and SSE is connected, disconnect
              get().disconnectSSE()
            }
          },
          { fireImmediately: true },
        )
      },

      initializeDownloadTracking: async (): Promise<void> => {

        const state = get()
        if (state.isInitialized) {
          return
        }

        try {
          // Load existing downloads
          await loadExistingDownloads(set)

          // Set up the subscription tracking
          get().setupDownloadTracking()

          set({ isInitialized: true })
        } catch (error) {
          console.error('Failed to initialize download tracking:', error)
        }
      },

      // Abort the module-scope SSE controller on store destroy. (audit 09 B-8)
      __destroy__: () => {
        if (sseAbortController) {
          sseAbortController.abort()
          sseAbortController = null
        }
      },
    }),
  ),
)

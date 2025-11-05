import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  DownloadFromRepositoryRequest,
  DownloadInstance,
} from '@/api-client/types'
import { loadModelsForProvider } from './store'

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
}

export const useLlmModelDownloadStore = create<LlmModelDownloadState>()(
  subscribeWithSelector(
    (): LlmModelDownloadState => ({
      // Initial state
      downloads: [],
      sseConnected: false,
      sseError: null,
      reconnectAttempts: 0,
      isInitialized: false,
      __init__: {
        downloads: () => initializeDownloadTracking(),
      },
    }),
  ),
)

// Download model from repository using new API
export const downloadLlmModelFromRepository = async (
  request: DownloadFromRepositoryRequest,
  onStart?: (downloadId: string) => void,
): Promise<{ downloadId: string }> => {
  try {
    // Call the new initiate download endpoint that returns immediately
    const downloadInstance = await ApiClient.LlmModel.download(request)

    // Add to downloads array
    useLlmModelDownloadStore.setState(state => ({
      downloads: [...state.downloads, downloadInstance],
    }))

    // Call onStart callback with the download ID
    onStart?.(downloadInstance.id)

    // Set up download tracking subscription if not already done
    console.log(
      '[Download] Setting up download tracking after adding download:',
      downloadInstance.id,
    )
    setupDownloadTracking()

    return { downloadId: downloadInstance.id }
  } catch (error) {
    console.error('Failed to initiate download:', error)
    throw error
  }
}

// Cancel download
export const cancelLlmModelDownload = async (
  downloadId: string,
): Promise<void> => {
  try {
    // Call backend to cancel download
    await ApiClient.LlmModel.cancelDownload({ download_id: downloadId })

    // Remove from local state immediately (backend will send update via SSE)
    useLlmModelDownloadStore.setState(state => ({
      downloads: state.downloads.filter(download => download.id !== downloadId),
    }))
  } catch (error) {
    console.error('Failed to cancel download:', error)
    throw error
  }
}

// Delete download
export const deleteLlmModelDownload = async (
  downloadId: string,
): Promise<void> => {
  try {
    // Call backend to delete download
    await ApiClient.LlmModel.deleteDownload({ download_id: downloadId })

    // Remove from local state
    useLlmModelDownloadStore.setState(state => ({
      downloads: state.downloads.filter(download => download.id !== downloadId),
    }))
  } catch (error) {
    console.error('Failed to delete download:', error)
    throw error
  }
}

export const clearLlmModelDownload = (downloadId: string): void => {
  useLlmModelDownloadStore.setState(state => ({
    downloads: state.downloads.filter(download => download.id !== downloadId),
  }))
}

export const clearAllLlmModelDownloads = (): void => {
  useLlmModelDownloadStore.setState({ downloads: [] })
}

export const getAllActiveDownloads = (): DownloadInstance[] => {
  const state = useLlmModelDownloadStore.getState()
  return state.downloads.filter(
    download =>
      download.status === 'downloading' || download.status === 'pending',
  )
}

export const findDownloadById = (
  downloadId: string,
): DownloadInstance | undefined => {
  return useLlmModelDownloadStore
    .getState()
    .downloads.find(download => download.id === downloadId)
}

// SSE abort controller for connection management
let sseAbortController: AbortController | null = null

// Subscribe to download progress updates via SSE using ApiClient
export const subscribeToDownloadProgress = async (): Promise<void> => {
  const state = useLlmModelDownloadStore.getState()

  // Don't reconnect if already connected
  if (state.sseConnected || sseAbortController) {
    console.log('SSE already connected')
    return
  }

  console.log('Subscribing to download progress updates via ApiClient...')

  try {
    // Call ApiClient with SSE handlers
    await ApiClient.LlmModel.subscribeDownloadProgress(undefined, {
      SSE: {
        __init: ({ abortController }) => {
          // Store abort controller for manual disconnection
          sseAbortController = abortController
          console.log('SSE connection initialized')
          useLlmModelDownloadStore.setState({
            sseConnected: true,
            sseError: null,
            reconnectAttempts: 0,
          })
        },

        connected: (data: { message?: string }) => {
          console.log('SSE connected:', data)
        },

        update: (updates: any[]) => {
          console.log('SSE update:', updates)

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
            console.log(
              '[Download] Refreshing models for providers:',
              providerIds,
            )
            for (const providerId of providerIds) {
              void loadModelsForProvider(providerId)
            }
          }

          useLlmModelDownloadStore.setState(state => {
            const updatedDownloads = state.downloads.map(download => {
              const update = updates.find((u: any) => u.id === download.id)
              return update ? { ...download, ...update } : download
            })

            // Filter out cancelled and completed downloads before updating state
            const filteredDownloads = updatedDownloads.filter(
              download =>
                download.status !== 'cancelled' &&
                download.status !== 'completed',
            )

            return { downloads: filteredDownloads }
          })
        },

        complete: (data: string) => {
          console.log('SSE complete:', data)

          // Get provider IDs from all downloads before they're filtered out
          const allDownloads = useLlmModelDownloadStore.getState().downloads
          const providerIds = [
            ...new Set(
              allDownloads
                .map(d => d.provider_id)
                .filter((id): id is string => !!id),
            ),
          ]

          // Refresh models for all providers that had downloads
          console.log(
            '[Download] Refreshing models for providers on complete:',
            providerIds,
          )
          for (const providerId of providerIds) {
            void loadModelsForProvider(providerId)
          }

          // Disconnect and reload downloads
          disconnectSSE()
          void loadExistingDownloads()
        },

        error: (errorMessage: string) => {
          console.error('SSE error:', errorMessage)
          useLlmModelDownloadStore.setState({
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

    const state = useLlmModelDownloadStore.getState()
    const attempts = state.reconnectAttempts + 1
    const maxAttempts = 5

    if (attempts < maxAttempts) {
      console.log(`Reconnection attempt ${attempts}/${maxAttempts}`)
      useLlmModelDownloadStore.setState({
        sseConnected: false,
        sseError: 'Connection lost, reconnecting...',
        reconnectAttempts: attempts,
      })

      // Retry after 3 seconds
      setTimeout(() => {
        void subscribeToDownloadProgress()
      }, 3000)
    } else {
      console.error('Max reconnection attempts reached')
      useLlmModelDownloadStore.setState({
        sseConnected: false,
        sseError: 'Failed to connect to download updates',
        reconnectAttempts: attempts,
      })
    }
  }
}

// Disconnect SSE
export const disconnectSSE = (): void => {
  console.log('Disconnecting SSE...')

  if (sseAbortController) {
    sseAbortController.abort()
    sseAbortController = null
  }

  useLlmModelDownloadStore.setState({
    sseConnected: false,
    reconnectAttempts: 0,
  })
}

// Load existing downloads from server
const loadExistingDownloads = async (): Promise<void> => {
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

    useLlmModelDownloadStore.setState({
      downloads,
    })
  } catch (error) {
    console.error('Failed to load downloads:', error)
  }
}

// Set up download tracking subscription
let isSubscriptionSetup = false
const setupDownloadTracking = (): void => {
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

      if (
        activeDownloads.length > 0 &&
        !useLlmModelDownloadStore.getState().sseConnected
      ) {
        // We have active downloads but no SSE connection, establish one
        console.log(
          '[Download] Active downloads detected, establishing SSE connection',
        )
        void subscribeToDownloadProgress()
      } else if (
        activeDownloads.length === 0 &&
        useLlmModelDownloadStore.getState().sseConnected
      ) {
        // No active downloads and SSE is connected, disconnect
        disconnectSSE()
      }
    },
    { fireImmediately: true },
  )
}

// Initialize download tracking
export const initializeDownloadTracking = async (): Promise<void> => {
  console.log('Initializing download tracking...')

  const state = useLlmModelDownloadStore.getState()
  if (state.isInitialized) {
    return
  }

  try {
    // Load existing downloads
    await loadExistingDownloads()

    // Set up the subscription tracking
    setupDownloadTracking()

    useLlmModelDownloadStore.setState({ isInitialized: true })
  } catch (error) {
    console.error('Failed to initialize download tracking:', error)
  }
}

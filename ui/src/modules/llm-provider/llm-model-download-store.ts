import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  DownloadFromRepositoryRequest,
  DownloadInstance,
} from '@/api-client/types'

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
    setupDownloadTracking()

    return { downloadId: downloadInstance.id }
  } catch (error) {
    console.error('Failed to initiate download:', error)
    throw error
  }
}

// TODO: Backend needs to implement DELETE /api/llm-models/downloads/{id}/cancel endpoint
export const cancelLlmModelDownload = async (
  downloadId: string,
): Promise<void> => {
  try {
    // TODO: When backend implements this endpoint, replace with:
    // await ApiClient.LlmModel.cancelDownload({ download_id: downloadId })
    console.warn(
      'Cancel download endpoint not yet implemented in backend:',
      downloadId,
    )

    // Remove from local state immediately
    useLlmModelDownloadStore.setState(state => ({
      downloads: state.downloads.filter(download => download.id !== downloadId),
    }))
  } catch (error) {
    console.error('Failed to cancel download:', error)
    throw error
  }
}

// TODO: Backend needs to implement DELETE /api/llm-models/downloads/{id} endpoint
export const deleteLlmModelDownload = async (
  downloadId: string,
): Promise<void> => {
  try {
    // TODO: When backend implements this endpoint, replace with:
    // await ApiClient.LlmModel.deleteDownload({ download_id: downloadId })
    console.warn(
      'Delete download endpoint not yet implemented in backend:',
      downloadId,
    )

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

// SSE Subscription Management
let sseReconnectTimeout: ReturnType<typeof setTimeout> | null = null
const MAX_RECONNECT_ATTEMPTS = 5
const RECONNECT_DELAY = 3000

// TODO: Backend needs to implement GET /api/llm-models/downloads/progress SSE endpoint
// Subscribe to download progress updates via SSE
export const subscribeToDownloadProgress = async (): Promise<void> => {
  const state = useLlmModelDownloadStore.getState()

  // If already connected, don't create another connection
  if (state.sseConnected) {
    return
  }

  try {
    console.log('Establishing SSE connection for download progress')

    useLlmModelDownloadStore.setState({
      sseConnected: true,
      sseError: null,
      reconnectAttempts: 0,
    })

    // TODO: When backend implements SSE endpoint, replace with:
    // await ApiClient.LlmModel.subscribeDownloadProgress(undefined, {
    //   SSE: {
    //     connected: data => { ... },
    //     update: data => { ... },
    //     complete: data => { ... },
    //     error: data => { ... },
    //     default: (event, data) => { ... },
    //   },
    // })
    console.warn('SSE download progress endpoint not yet implemented in backend')

    // For now, just mark as disconnected
    useLlmModelDownloadStore.setState({
      sseConnected: false,
      sseError: 'SSE endpoint not yet implemented',
    })
  } catch (error) {
    console.error('Failed to establish SSE connection:', error)
    useLlmModelDownloadStore.setState({
      sseConnected: false,
      sseError: error instanceof Error ? error.message : 'Failed to connect',
    })

    // Attempt reconnection if we have active downloads
    const activeDownloads = getAllActiveDownloads()
    if (activeDownloads.length > 0) {
      handleReconnection()
    }
  }
}

// Disconnect SSE connection
export const disconnectSSE = (): void => {
  useLlmModelDownloadStore.setState({
    sseConnected: false,
    sseError: null,
  })

  // Clear any pending reconnection timeout
  if (sseReconnectTimeout) {
    clearTimeout(sseReconnectTimeout)
    sseReconnectTimeout = null
  }
}

// Handle reconnection logic
const handleReconnection = (): void => {
  const { reconnectAttempts } = useLlmModelDownloadStore.getState()

  if (reconnectAttempts >= MAX_RECONNECT_ATTEMPTS) {
    console.error('Max reconnection attempts reached')
    useLlmModelDownloadStore.setState({
      sseError: 'Failed to reconnect after multiple attempts',
    })
    return
  }

  // Clear existing timeout if any
  if (sseReconnectTimeout) {
    clearTimeout(sseReconnectTimeout)
  }

  // Increment reconnect attempts
  useLlmModelDownloadStore.setState(state => ({
    reconnectAttempts: state.reconnectAttempts + 1,
  }))

  // Attempt reconnection after delay
  sseReconnectTimeout = setTimeout(async () => {
    console.log(
      `Attempting SSE reconnection (${reconnectAttempts + 1}/${MAX_RECONNECT_ATTEMPTS})`,
    )
    await subscribeToDownloadProgress()
  }, RECONNECT_DELAY)
}

// TODO: Backend needs to implement GET /api/llm-models/downloads endpoint
// Load existing downloads from server
const loadExistingDownloads = async (): Promise<void> => {
  try {
    // TODO: When backend implements this endpoint, replace with:
    // const response = await ApiClient.LlmModel.listAllDownloads({})
    // const downloads = response.downloads.filter(download =>
    //   ['pending', 'downloading', 'failed'].includes(download.status),
    // )
    // useLlmModelDownloadStore.setState({ downloads })
    console.warn('List downloads endpoint not yet implemented in backend')

    // For now, just set empty array
    useLlmModelDownloadStore.setState({ downloads: [] })
  } catch (error) {
    console.error('Failed to load existing downloads:', error)
  }
}

// Set up download tracking subscription (called automatically when store changes)
let isSubscriptionSetup = false
const setupDownloadTracking = (): void => {
  if (isSubscriptionSetup) return
  isSubscriptionSetup = true

  // Subscribe to store changes to manage SSE connection
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
        void subscribeToDownloadProgress()
      } else if (
        activeDownloads.length === 0 &&
        useLlmModelDownloadStore.getState().sseConnected
      ) {
        // No active downloads and SSE is connected, disconnect
        disconnectSSE()
      }
    },
  )
}

// Initialize download tracking after authentication with provider read permission
export const initializeDownloadTracking = async (): Promise<void> => {
  const state = useLlmModelDownloadStore.getState()
  if (state.isInitialized) {
    return
  }

  try {
    // Set up the subscription tracking
    setupDownloadTracking()

    // Load existing downloads from server
    await loadExistingDownloads()

    useLlmModelDownloadStore.setState({ isInitialized: true })
  } catch (error) {
    console.error('Failed to initialize download tracking:', error)
  }
}

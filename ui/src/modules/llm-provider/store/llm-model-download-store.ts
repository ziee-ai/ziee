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

// SSE EventSource instance
let sseEventSource: EventSource | null = null

// Subscribe to download progress updates via SSE
export const subscribeToDownloadProgress = (): void => {
  const state = useLlmModelDownloadStore.getState()

  // Don't reconnect if already connected
  if (state.sseConnected || sseEventSource) {
    console.log('SSE already connected')
    return
  }

  console.log('Subscribing to download progress updates...')

  try {
    // Create EventSource connection
    const token = localStorage.getItem('auth_token')
    const url = `/api/llm-models/downloads/subscribe${token ? `?token=${token}` : ''}`
    sseEventSource = new EventSource(url)

    // Handle connection opened
    sseEventSource.addEventListener('open', () => {
      console.log('SSE connection established')
      useLlmModelDownloadStore.setState({
        sseConnected: true,
        sseError: null,
        reconnectAttempts: 0,
      })
    })

    // Handle 'connected' event
    sseEventSource.addEventListener('connected', (event) => {
      const data = JSON.parse(event.data)
      console.log('SSE connected event:', data)
    })

    // Handle 'update' event (progress updates)
    sseEventSource.addEventListener('update', (event) => {
      const updates = JSON.parse(event.data)
      console.log('SSE update event:', updates)

      useLlmModelDownloadStore.setState(state => {
        const updatedDownloads = state.downloads.map(download => {
          const update = updates.find((u: any) => u.id === download.id)
          return update ? { ...download, ...update } : download
        })
        return { downloads: updatedDownloads }
      })
    })

    // Handle 'complete' event
    sseEventSource.addEventListener('complete', (event) => {
      const data = JSON.parse(event.data)
      console.log('SSE complete event:', data)

      // Disconnect and reload downloads
      disconnectSSE()
      void loadExistingDownloads()
    })

    // Handle 'error' event from server
    sseEventSource.addEventListener('error_event', (event) => {
      const data = JSON.parse(event.data)
      console.error('SSE error event:', data)

      useLlmModelDownloadStore.setState(state => {
        const updatedDownloads = state.downloads.map(download =>
          download.id === data.download_id
            ? { ...download, status: 'failed' as const, error_message: data.message }
            : download
        )
        return { downloads: updatedDownloads }
      })
    })

    // Handle connection errors
    sseEventSource.onerror = (error) => {
      console.error('SSE connection error:', error)

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

        // Close current connection
        sseEventSource?.close()
        sseEventSource = null

        // Retry after 3 seconds
        setTimeout(() => {
          subscribeToDownloadProgress()
        }, 3000)
      } else {
        console.error('Max reconnection attempts reached')
        useLlmModelDownloadStore.setState({
          sseConnected: false,
          sseError: 'Failed to connect to download updates',
          reconnectAttempts: attempts,
        })
        disconnectSSE()
      }
    }
  } catch (error) {
    console.error('Failed to subscribe to download progress:', error)
    useLlmModelDownloadStore.setState({
      sseConnected: false,
      sseError: 'Failed to establish connection',
    })
  }
}

// Disconnect SSE
export const disconnectSSE = (): void => {
  console.log('Disconnecting SSE...')

  if (sseEventSource) {
    sseEventSource.close()
    sseEventSource = null
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

    // Filter out completed/cancelled downloads
    const activeDownloads = response.downloads.filter(d =>
      d.status === 'downloading' || d.status === 'pending'
    )

    useLlmModelDownloadStore.setState({
      downloads: activeDownloads,
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
  useLlmModelDownloadStore.subscribe(
    state => state.downloads,
    downloads => {
      const activeDownloads = downloads.filter(
        d => d.status === 'downloading' || d.status === 'pending',
      )

      const { sseConnected } = useLlmModelDownloadStore.getState()

      if (activeDownloads.length > 0 && !sseConnected) {
        console.log('Active downloads detected, connecting SSE...')
        subscribeToDownloadProgress()
      } else if (activeDownloads.length === 0 && sseConnected) {
        console.log('No active downloads, disconnecting SSE...')
        disconnectSSE()
      }
    },
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

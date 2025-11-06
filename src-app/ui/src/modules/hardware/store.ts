import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  HardwareInfo,
  HardwareInfoResponse,
  HardwareUsageUpdate,
} from '@/api-client/types'

interface HardwareState {
  // Static hardware information
  hardwareInfo: HardwareInfo | null
  hardwareLoading: boolean
  hardwareError: string | null
  hardwareInitialized: boolean

  // Real-time usage data
  currentUsage: HardwareUsageUpdate | null
  usageLoading: boolean
  usageError: string | null

  // SSE connection state
  sseConnected: boolean
  sseConnecting: boolean
  sseError: string | null

  __init__: {
    hardwareInfo: () => Promise<void>
  }

  // Actions
  loadHardwareInfo: () => Promise<void>
  clearHardwareError: () => void
  subscribeToHardwareUsage: () => Promise<void>
  disconnectHardwareUsage: () => void
}

// SSE Subscription Management for real-time usage monitoring
let sseAbortController: AbortController | null = null
let isIntentionallyDisconnecting = false
let isCurrentlyConnecting = false // Module-level flag to prevent StrictMode double-mounting issues
let lastDisconnectTime = 0 // Timestamp of last disconnect to prevent immediate reconnection

export const useHardwareStore = create<HardwareState>()(
  subscribeWithSelector(
    (set, get): HardwareState => ({
      // Static hardware info
      hardwareInfo: null,
      hardwareLoading: false,
      hardwareError: null,
      hardwareInitialized: false,

      // Real-time usage data
      currentUsage: null,
      usageLoading: false,
      usageError: null,

      // SSE connection state
      sseConnected: false,
      sseConnecting: false,
      sseError: null,

      __init__: {
        hardwareInfo: async () => {
          await get().loadHardwareInfo()
        },
      },

      // Actions
      loadHardwareInfo: async () => {
        const state = get()
        if (state.hardwareInitialized || state.hardwareLoading) {
          return
        }

        set({ hardwareLoading: true, hardwareError: null })

        try {
          const response: HardwareInfoResponse =
            await ApiClient.Hardware.info(undefined)
          set({
            hardwareInfo: response.hardware,
            hardwareInitialized: true,
            hardwareLoading: false,
            hardwareError: null,
          })
        } catch (error) {
          console.error('Hardware info loading failed:', error)
          set({
            hardwareLoading: false,
            hardwareError: error instanceof Error ? error.message : 'Unknown error',
            hardwareInitialized: false,
          })
          throw error
        }
      },

      clearHardwareError: () => {
        set({
          hardwareError: null,
          usageError: null,
          sseError: null,
        })
      },

      subscribeToHardwareUsage: async () => {
        // Prevent reconnection immediately after disconnect (StrictMode remount protection)
        const timeSinceDisconnect = Date.now() - lastDisconnectTime
        if (timeSinceDisconnect < 200 && lastDisconnectTime > 0) {
          console.log(
            `Hardware SSE: Skipping connection attempt (disconnected ${timeSinceDisconnect}ms ago)`,
          )
          return
        }

        // Check module-level flag first to prevent StrictMode double-mounting issues
        if (isCurrentlyConnecting || sseAbortController !== null) {
          console.log(
            'Hardware SSE: Skipping connection attempt (already connecting or connected)',
          )
          return
        }

        const state = get()

        // Additional check against store state
        if (state.sseConnected || state.sseConnecting) {
          console.log(
            'Hardware SSE: Skipping connection attempt (store shows connected/connecting)',
          )
          return
        }

        // Mark as connecting immediately (both module-level and store)
        isCurrentlyConnecting = true
        set({
          sseConnecting: true,
          sseError: null,
          usageLoading: true,
        })

        try {
          console.log('Establishing SSE connection for hardware usage monitoring')

          // Reset disconnection flag
          isIntentionallyDisconnecting = false

          await ApiClient.Hardware.stream(undefined, {
            SSE: {
              __init: data => {
                sseAbortController = data.abortController
                isCurrentlyConnecting = false // Connection established
                console.log('Hardware SSE AbortController initialized')
                set({
                  sseConnected: true,
                  sseConnecting: false,
                })
              },
              connected: data => {
                console.log(
                  'Hardware usage monitoring connected:',
                  data.message || 'Connected',
                )
                isCurrentlyConnecting = false
                set({
                  usageLoading: false,
                  sseError: null,
                  sseConnecting: false,
                })
              },
              update: data => {
                set({
                  currentUsage: data,
                  usageLoading: false,
                  usageError: null,
                })
              },
              default: (event, data) => {
                console.log('Unknown hardware SSE event:', event, data)
              },
            },
          })
        } catch (error) {
          // Reset connecting flag on error
          isCurrentlyConnecting = false

          // Ignore AbortErrors as they are expected during cleanup/disconnection
          if (error instanceof Error && error.name === 'AbortError') {
            if (isIntentionallyDisconnecting) {
              console.log(
                'Hardware SSE connection was intentionally aborted during cleanup',
              )
            } else {
              console.log('Hardware SSE connection was aborted (unexpected)')
            }
            set({
              sseConnected: false,
              sseConnecting: false,
              usageLoading: false,
            })
            return
          }

          console.error('Failed to establish hardware SSE connection:', error)
          set({
            sseConnected: false,
            sseConnecting: false,
            sseError: error instanceof Error ? error.message : 'Failed to connect',
            usageLoading: false,
          })
        }
      },

      disconnectHardwareUsage: () => {
        console.log('Disconnecting hardware usage monitoring')

        // Set flag to indicate intentional disconnection
        isIntentionallyDisconnecting = true

        // Record disconnect time to prevent immediate reconnection
        lastDisconnectTime = Date.now()

        // Abort the SSE connection if AbortController is available
        if (sseAbortController) {
          sseAbortController.abort()
          sseAbortController = null
          console.log('Hardware SSE connection aborted')
        }

        // Reset module-level flags
        isCurrentlyConnecting = false

        set({
          sseConnected: false,
          sseConnecting: false,
          sseError: null,
          currentUsage: null,
          usageLoading: false,
        })

        // Reset flag after disconnection
        isIntentionallyDisconnecting = false
      },
    }),
  ),
)

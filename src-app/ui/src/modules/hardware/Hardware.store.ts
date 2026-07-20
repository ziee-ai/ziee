import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type {
  HardwareInfo,
  HardwareInfoResponse,
  HardwareUsageUpdate,
} from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

// SSE Subscription Management for real-time usage monitoring (module-scope:
// not serializable / reactive).
let sseAbortController: AbortController | null = null
let isIntentionallyDisconnecting = false
let isCurrentlyConnecting = false // prevents StrictMode double-mount races
let lastDisconnectTime = 0 // guards against immediate reconnect

export const Hardware = defineStore('Hardware', {
  state: {
    // Static hardware information
    hardwareInfo: null as HardwareInfo | null,
    hardwareLoading: false,
    hardwareError: null as string | null,
    hardwareInitialized: false,
    // Real-time usage data
    currentUsage: null as HardwareUsageUpdate | null,
    usageLoading: false,
    usageError: null as string | null,
    // SSE connection state
    sseConnected: false,
    sseConnecting: false,
    sseError: null as string | null,
  },
  actions: (set, get) => ({
    loadHardwareInfo: async () => {
      const state = get()
      if (state.hardwareInitialized || state.hardwareLoading) return
      set({ hardwareLoading: true, hardwareError: null })
      try {
        const response: HardwareInfoResponse = await ApiClient.Hardware.info(undefined)
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
      set({ hardwareError: null, usageError: null, sseError: null })
    },
    subscribeToHardwareUsage: async () => {
      // Prevent reconnection immediately after disconnect (StrictMode remount).
      const timeSinceDisconnect = Date.now() - lastDisconnectTime
      if (timeSinceDisconnect < 200 && lastDisconnectTime > 0) {
        console.log(
          `Hardware SSE: Skipping connection attempt (disconnected ${timeSinceDisconnect}ms ago)`,
        )
        return
      }
      if (isCurrentlyConnecting || sseAbortController !== null) {
        console.log('Hardware SSE: Skipping connection attempt (already connecting or connected)')
        return
      }
      const state = get()
      if (state.sseConnected || state.sseConnecting) {
        console.log('Hardware SSE: Skipping connection attempt (store shows connected/connecting)')
        return
      }
      isCurrentlyConnecting = true
      set({ sseConnecting: true, sseError: null, usageLoading: true })
      try {
        console.log('Establishing SSE connection for hardware usage monitoring')
        isIntentionallyDisconnecting = false
        await ApiClient.Hardware.stream(undefined, {
          SSE: {
            __init: data => {
              sseAbortController = data.abortController
              isCurrentlyConnecting = false
              console.log('Hardware SSE AbortController initialized')
              set({ sseConnected: true, sseConnecting: false })
            },
            connected: data => {
              console.log('Hardware usage monitoring connected:', data.message || 'Connected')
              isCurrentlyConnecting = false
              set({ usageLoading: false, sseError: null, sseConnecting: false })
            },
            update: data => {
              set({ currentUsage: data, usageLoading: false, usageError: null })
            },
            default: (event, data) => {
              console.log('Unknown hardware SSE event:', event, data)
            },
          },
        })
      } catch (error) {
        isCurrentlyConnecting = false
        // Clear the AbortController handle. The API core fires `__init` (which
        // stashes `sseAbortController`) BEFORE checking `response.ok`, so a
        // failed connection can leave it non-null with no live stream; if not
        // nulled, the guard above permanently blocks every reconnect.
        sseAbortController = null
        // Ignore AbortErrors (expected during cleanup/disconnection).
        if (error instanceof Error && error.name === 'AbortError') {
          if (isIntentionallyDisconnecting) {
            console.log('Hardware SSE connection was intentionally aborted during cleanup')
          } else {
            console.log('Hardware SSE connection was aborted (unexpected)')
          }
          set({ sseConnected: false, sseConnecting: false, usageLoading: false })
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
      isIntentionallyDisconnecting = true
      lastDisconnectTime = Date.now()
      if (sseAbortController) {
        sseAbortController.abort()
        sseAbortController = null
        console.log('Hardware SSE connection aborted')
      }
      isCurrentlyConnecting = false
      set({
        sseConnected: false,
        sseConnecting: false,
        sseError: null,
        currentUsage: null,
        usageLoading: false,
      })
      isIntentionallyDisconnecting = false
    },
  }),
  init: ({ actions, onCleanup }) => {
    // `/api/hardware/info` requires hardware::read (not user-held). A user
    // reaching /hardware-monitor via hardware::monitor alone would otherwise
    // 403 on this eager fetch at store-mount. SSE connect is gated by the
    // route perm separately.
    if (hasPermissionNow(Permissions.HardwareRead)) {
      void actions.loadHardwareInfo()
    }
    // Abort the module-scope SSE controller on store destroy so it doesn't
    // outlive the store (the proxy may destroy it after a 5s grace period; an
    // in-flight SSE keeps running otherwise and blocks reconnect). (audit 09 B-8)
    onCleanup(() => {
      if (sseAbortController) {
        sseAbortController.abort()
        sseAbortController = null
      }
      isCurrentlyConnecting = false
      isIntentionallyDisconnecting = false
    })
  },
})

export const useHardwareStore = Hardware.store

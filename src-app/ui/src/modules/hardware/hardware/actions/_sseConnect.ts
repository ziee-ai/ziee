import { ApiClient } from '@/api-client'
import type { HardwareGet, HardwareSet } from '../state'

// ── Module-scope SSE state (shared by _sseConnect, subscribeToHardwareUsage,
// disconnectHardwareUsage, and the onCleanup hook in index.ts). These are
// intentionally NOT in the Zustand store because they track in-flight
// connection lifecycle across strict-Mode remounts and store proxies. ────────
let sseAbortController: AbortController | null = null
let isIntentionallyDisconnecting = false
let isCurrentlyConnecting = false
let lastDisconnectTime = 0

/** Reset connection-lifecycle guards only (not lastDisconnectTime).
 *  Called by the store onCleanup hook so dead proxies don't permanently
 *  block reconnects. */
export function resetSseGuards(): void {
  sseAbortController = null
  isCurrentlyConnecting = false
  isIntentionallyDisconnecting = false
}

/** Record the time of the latest disconnect.  Called by disconnect action
 *  to enforce the StrictMode remount cooldown. */
export function recordDisconnectTime(): number {
  lastDisconnectTime = Date.now()
  return lastDisconnectTime
}

/** Get the elapsed ms since the latest disconnect. */
export function elapsedSinceDisconnect(): number {
  return Date.now() - lastDisconnectTime
}

/** Internal SSE connection handler. Mutates module-scope variables. */
export default (set: HardwareSet, get: HardwareGet) =>
  async () => {
    // Prevent reconnection immediately after disconnect (StrictMode remount).
    if (elapsedSinceDisconnect() < 200 && lastDisconnectTime > 0) {
      console.log(
        `Hardware SSE: Skipping connection attempt (disconnected ${Date.now() - lastDisconnectTime}ms ago)`,
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
  }

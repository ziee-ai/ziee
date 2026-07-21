/** Shared module-scoped SSE state.
 *
 * AbortController / reconnect state lives outside immer (AbortController
 * isn't draftable). Both `subscribeToInstallProgress` and `cleanupSse`
 * read/write the same module-scoped vars, so we colocate them here.
 */

export const sseState = {
  controller: null as AbortController | null,
  reconnectTimer: null as ReturnType<typeof setTimeout> | null,
  reconnectAttempts: 0,
  maxReconnectAttempts: 5,
  reconnectDelayMs: 3000,
}

export function cleanupSseState() {
  sseState.controller?.abort()
  sseState.controller = null
  if (sseState.reconnectTimer) {
    clearTimeout(sseState.reconnectTimer)
    sseState.reconnectTimer = null
  }
  sseState.reconnectAttempts = 0
}

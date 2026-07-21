import type { LlmModelDownloadSet, LlmModelDownloadGet } from '../state'

export default (set: LlmModelDownloadSet, _get: LlmModelDownloadGet) =>
  async (): Promise<void> => {
    // Abort module-scope SSE controller (mirror original disconnectSSE).
    const controller = (globalThis as Record<string, unknown>).__LLM_DL_SSE_ABORT as
      | AbortController
      | undefined
    if (controller) {
      controller.abort()
      ;(globalThis as Record<string, unknown>).__LLM_DL_SSE_ABORT = undefined
    }
    set({ sseConnected: false, reconnectAttempts: 0 })
  }

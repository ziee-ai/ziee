import { ApiClient } from '@/api-client'
import { useLlmProviderStore } from '@/modules/llm-provider/stores/LlmProvider.store'
import { useLlmModelDownloadStore } from '@/modules/llm-provider/stores/llmModelDownload'
import type {
  DownloadInstance,
  DownloadProgressUpdate,
  SSEDownloadProgressConnectedData,
} from '@/api-client/types'
import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'
import loadExistingDownloadsFactory from './_loadExistingDownloads'
import {
  emitLlmModelDownloadCompleted,
  emitLlmModelDownloadFailed,
} from '@/modules/llm-provider/events/emitters'

export default (set: LlmModelDownloadSet, get: LlmModelDownloadGet) => {
  const loadExistingDownloads = loadExistingDownloadsFactory(set, get)

  const action: () => Promise<void> = async () => {
    const state = get()
    if (state.sseConnected) return

    try {
      await ApiClient.LlmModel.subscribeDownloadProgress(undefined, {
        SSE: {
          __init: ({ abortController }) => {
            // Signal the abort controller so onCleanup can abort it.
            ;(globalThis as Record<string, unknown>).__LLM_DL_SSE_ABORT = abortController
            set({ sseConnected: true, sseError: null, reconnectAttempts: 0 })
          },
          connected: (_data: SSEDownloadProgressConnectedData) => {},
          update: (updates: DownloadProgressUpdate[]) => {
            const prevState = get()
            const prevStatusById = new Map<string, string>(
              prevState.downloads.map((d: DownloadInstance) => [d.id, d.status]),
            )
            const newlyCompleted = updates.filter((u) => u.status === 'completed')
            if (newlyCompleted.length > 0) {
              const providerIds = [
                ...new Set(
                  newlyCompleted
                    .map((d) => d.provider_id)
                    .filter((id): id is string => !!id),
                ),
              ]
              for (const providerId of providerIds) {
                void useLlmProviderStore.getState().loadModelsForProvider(providerId)
              }
            }
            for (const u of updates) {
              if (!u.id || typeof u.status !== 'string') continue
              const prev = prevStatusById.get(u.id)
              if (prev === u.status) continue
              const isNewlyTerminal =
                (u.status === 'completed' || u.status === 'failed') && prev !== undefined
              if (!isNewlyTerminal) continue
              const priorRow = prevState.downloads.find((d) => d.id === u.id)
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
            set((state) => {
              const updatedDownloads = state.downloads.map((download) => {
                const update = updates.find((u) => u.id === download.id)
                return update
                  ? ({ ...download, ...update } as DownloadInstance)
                  : download
              })
              const filteredDownloads = updatedDownloads.filter(
                (download) => download.status !== 'cancelled' && download.status !== 'completed',
              )
              return { downloads: filteredDownloads }
            })
          },
          complete: (_data: string) => {
            const allDownloads = get().downloads
            const providerIds = [
              ...new Set(allDownloads.map((d) => d.provider_id).filter((id): id is string => !!id)),
            ]
            for (const providerId of providerIds) {
              void useLlmProviderStore.getState().loadModelsForProvider(providerId)
            }
            void useLlmModelDownloadStore.getState().disconnectSSE()
            void loadExistingDownloads()
          },
          error: (errorMessage: string) => {
            console.error('SSE error:', errorMessage)
            set({ sseError: errorMessage, sseConnected: false })
          },
          default: (event: string, data: unknown) => {
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
          void action()
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

  return action
}

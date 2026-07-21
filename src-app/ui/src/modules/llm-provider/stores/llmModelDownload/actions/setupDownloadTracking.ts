import { useLlmModelDownloadStore } from '@/modules/llm-provider/stores/llmModelDownload'
import type { DownloadInstance } from '@/api-client/types'
import type { LlmModelDownloadGet, LlmModelDownloadSet } from '../state'

export default (_set: LlmModelDownloadSet, get: LlmModelDownloadGet) => {
  let isSubscriptionSetup = false

  return async (): Promise<void> => {
    if (isSubscriptionSetup) return
    isSubscriptionSetup = true
    useLlmModelDownloadStore.subscribe(
      (state) => state.downloads,
      (downloads: DownloadInstance[]) => {
        const activeDownloads = downloads.filter(
          (d) => d.status === 'downloading' || d.status === 'pending',
        )
        const state = get()
        if (activeDownloads.length > 0 && !state.sseConnected) {
          void useLlmModelDownloadStore.getState().subscribeToDownloadProgress()
        } else if (activeDownloads.length === 0 && state.sseConnected) {
          void useLlmModelDownloadStore.getState().disconnectSSE()
        }
      },
      { fireImmediately: true },
    )
  }
}

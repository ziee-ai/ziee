import type { StoreSet } from '@ziee/framework/store-kit'
import type { DownloadInstance } from '@/api-client/types'

export const llmModelDownloadState = {
  downloads: [] as DownloadInstance[],
  sseConnected: false,
  sseError: null as string | null,
  reconnectAttempts: 0,
  isInitialized: false,
}

export type LlmModelDownloadState = typeof llmModelDownloadState
export type LlmModelDownloadSet = StoreSet<LlmModelDownloadState>
export type LlmModelDownloadGet = () => LlmModelDownloadState

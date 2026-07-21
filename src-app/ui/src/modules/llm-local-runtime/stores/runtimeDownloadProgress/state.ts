import type { StoreSet } from '@ziee/framework/store-kit'
import type { DownloadSnapshot } from '@/api-client/types'

export const runtimeDownloadProgressState = {
  activeByKey: new Map<string, DownloadSnapshot>(),
  loadingActive: false,
  error: null as string | null,
}

export type RuntimeDownloadProgressState = typeof runtimeDownloadProgressState
export type RuntimeDownloadProgressSet = StoreSet<RuntimeDownloadProgressState>
export type RuntimeDownloadProgressGet = () => RuntimeDownloadProgressState

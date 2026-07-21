import type { StoreSet } from '@ziee/framework/store-kit'
import type { DownloadSnapshot2 } from '@/api-client/types'

export const voiceDownloadProgressState = {
  activeByKey: new Map<string, DownloadSnapshot2>(),
  loadingActive: false,
  error: null as string | null,
}

export type VoiceDownloadProgressState = typeof voiceDownloadProgressState
export type VoiceDownloadProgressSet = StoreSet<VoiceDownloadProgressState>
export type VoiceDownloadProgressGet = () => VoiceDownloadProgressState

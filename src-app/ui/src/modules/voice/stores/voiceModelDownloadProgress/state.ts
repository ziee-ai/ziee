import type { StoreSet } from '@ziee/framework/store-kit'
import type { SnapshotDto } from '@/api-client/types'

export const voiceModelDownloadProgressState = {
  activeByKey: new Map<string, SnapshotDto>(),
  loadingActive: false,
  error: null as string | null,
}

export type VoiceModelDownloadProgressState = typeof voiceModelDownloadProgressState
export type VoiceModelDownloadProgressSet = StoreSet<VoiceModelDownloadProgressState>
export type VoiceModelDownloadProgressGet = () => VoiceModelDownloadProgressState

import type { StoreSet } from '@ziee/framework/store-kit'
import { type VoiceModel as VoiceModelRow, type VoiceModelStatus } from '@/api-client/types'

export const voiceModelState = {
  status: null as VoiceModelStatus | null,
  installed: [] as VoiceModelRow[],
  loading: false,
  loadingInstalled: false,
  activating: new Map<string, boolean>(),
  deleting: new Map<string, boolean>(),
  error: null as string | null,
}

export type VoiceModelState = typeof voiceModelState
export type VoiceModelSet = StoreSet<VoiceModelState>
export type VoiceModelGet = () => VoiceModelState

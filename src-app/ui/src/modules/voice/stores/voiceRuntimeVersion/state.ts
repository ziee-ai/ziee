import type { StoreSet } from '@ziee/framework/store-kit'
import type { RuntimeVersionResponse2 } from '@/api-client/types'

export const voiceRuntimeVersionState = {
  versions: [] as RuntimeVersionResponse2[],
  isInitialized: false,
  loading: false,
  settingDefault: new Map<string, boolean>(),
  deleting: new Map<string, boolean>(),
  error: null as string | null,
}

export type VoiceRuntimeVersionState = typeof voiceRuntimeVersionState
export type VoiceRuntimeVersionSet = StoreSet<VoiceRuntimeVersionState>
export type VoiceRuntimeVersionGet = () => VoiceRuntimeVersionState

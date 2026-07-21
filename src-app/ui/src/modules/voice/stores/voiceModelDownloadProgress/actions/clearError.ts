import type { VoiceModelDownloadProgressSet } from '../state'

export default (set: VoiceModelDownloadProgressSet) =>
  (): void =>
    set({ error: null })

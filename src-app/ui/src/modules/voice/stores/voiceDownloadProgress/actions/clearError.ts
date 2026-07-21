import type { VoiceDownloadProgressSet } from '../state'

export default (set: VoiceDownloadProgressSet) =>
  async (): Promise<void> => {
    set({ error: null })
  }

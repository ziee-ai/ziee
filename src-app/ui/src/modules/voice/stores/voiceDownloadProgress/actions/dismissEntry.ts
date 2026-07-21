import type { VoiceDownloadProgressSet } from '../state'

export default (set: VoiceDownloadProgressSet) =>
  async (key: string): Promise<void> => {
    set(state => {
      const next = new Map(state.activeByKey)
      next.delete(key)
      return { activeByKey: next }
    })
  }

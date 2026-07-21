import type { VoiceModelDownloadProgressSet } from '../state'

export default (set: VoiceModelDownloadProgressSet) =>
  (key: string): void =>
    set(state => {
      const next = new Map(state.activeByKey)
      next.delete(key)
      return { activeByKey: next }
    })

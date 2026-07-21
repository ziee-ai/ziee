import type { VoiceRuntimeVersionSet } from '../state'

export default (set: VoiceRuntimeVersionSet) => () => {
  set({ error: null })
}

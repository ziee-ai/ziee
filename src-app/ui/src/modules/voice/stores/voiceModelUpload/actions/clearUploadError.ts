import type { VoiceModelUploadSet, VoiceModelUploadGet } from '../state'

export default (set: VoiceModelUploadSet, _get: VoiceModelUploadGet) => {
  return () => {
    set({ uploadError: null })
  }
}

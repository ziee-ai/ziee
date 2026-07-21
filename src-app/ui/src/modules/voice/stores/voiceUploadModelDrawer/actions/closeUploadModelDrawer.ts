import type { VoiceUploadModelDrawerSet } from '../state'

export default (set: VoiceUploadModelDrawerSet) => {
  return async () => {
    set({ open: false })
  }
}

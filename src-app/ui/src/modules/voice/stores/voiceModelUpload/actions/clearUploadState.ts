import type { VoiceModelUploadSet, VoiceModelUploadGet } from '../state'

export default (set: VoiceModelUploadSet, _get: VoiceModelUploadGet) => {
  return () => {
    set({
      uploading: false,
      uploadProgress: [],
      overallUploadProgress: 0,
      uploadError: null,
    })
  }
}

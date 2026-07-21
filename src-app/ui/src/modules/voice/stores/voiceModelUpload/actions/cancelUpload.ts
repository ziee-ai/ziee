import { getCurrentUploadXhr, setCurrentUploadXhr } from '../state'
import type { VoiceModelUploadSet, VoiceModelUploadGet } from '../state'

export default (_set: VoiceModelUploadSet, _get: VoiceModelUploadGet) => {
  return () => {
    const xhr = getCurrentUploadXhr()
    if (xhr) {
      xhr.abort()
      setCurrentUploadXhr(null)
    }
  }
}

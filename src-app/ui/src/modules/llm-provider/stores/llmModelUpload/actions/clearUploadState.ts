import type { LlmModelGet, LlmModelSet } from '../state'

/** Reset all upload state to initial values */
export default (set: LlmModelSet, _get: LlmModelGet) => {
  return () => {
    set({ uploading: false, uploadProgress: [], overallUploadProgress: 0, uploadError: null })
  }
}

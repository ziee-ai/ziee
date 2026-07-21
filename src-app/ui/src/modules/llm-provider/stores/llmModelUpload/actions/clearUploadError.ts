import type { LlmModelGet, LlmModelSet } from '../state'

/** Clear the current upload error message */
export default (set: LlmModelSet, _get: LlmModelGet) => {
  return () => {
    set({ uploadError: null })
  }
}

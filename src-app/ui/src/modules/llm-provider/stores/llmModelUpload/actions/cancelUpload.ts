import type { LlmModelGet, LlmModelSet } from '../state'
import { getCurrentXhr } from './_uploadLocalModel'

/** Cancel in-flight XHR upload */
export default (_set: LlmModelSet, _get: LlmModelGet) => {
  return () => {
    const xhr = getCurrentXhr()
    if (xhr) {
      xhr.abort()
    }
  }
}

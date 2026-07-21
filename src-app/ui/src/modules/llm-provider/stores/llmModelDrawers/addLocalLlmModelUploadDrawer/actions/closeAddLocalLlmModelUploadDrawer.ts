import type { AddLocalLlmModelUploadDrawerSet } from '../state'

export default (
  set: AddLocalLlmModelUploadDrawerSet,
  _get: import('../state').AddLocalLlmModelUploadDrawerGet,
) => async () => {
  set(s => {
    s.open = false
    s.loading = false
    s.providerId = null
  })
}

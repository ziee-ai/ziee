import type { AddLocalLlmModelDownloadDrawerSet } from '../state'

export default (
  set: AddLocalLlmModelDownloadDrawerSet,
  _get: import('../state').AddLocalLlmModelDownloadDrawerGet,
) => async () => {
  set(s => {
    s.open = false
    s.loading = false
    s.providerId = null
  })
}

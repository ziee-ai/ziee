import type { AddLocalLlmModelDownloadDrawerSet } from '../state'

export default (
  set: AddLocalLlmModelDownloadDrawerSet,
  _get: import('../state').AddLocalLlmModelDownloadDrawerGet,
) => async (loading: boolean) => {
  set(s => {
    s.loading = loading
  })
}

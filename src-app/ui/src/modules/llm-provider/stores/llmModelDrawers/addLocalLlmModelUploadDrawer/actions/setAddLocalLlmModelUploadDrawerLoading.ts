import type { AddLocalLlmModelUploadDrawerSet } from '../state'

export default (
  set: AddLocalLlmModelUploadDrawerSet,
  _get: import('../state').AddLocalLlmModelUploadDrawerGet,
) => async (loading: boolean) => {
  set(s => {
    s.loading = loading
  })
}

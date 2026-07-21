import type { AddLocalLlmModelUploadDrawerSet } from '../state'

export default (
  set: AddLocalLlmModelUploadDrawerSet,
  _get: import('../state').AddLocalLlmModelUploadDrawerGet,
) =>
  async (providerId: string) => {
    set(s => {
      s.open = true
      s.providerId = providerId
    })
  }

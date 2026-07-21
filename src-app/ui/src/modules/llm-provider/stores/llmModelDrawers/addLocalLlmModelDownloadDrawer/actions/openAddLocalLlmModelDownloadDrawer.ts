import type { AddLocalLlmModelDownloadDrawerSet } from '../state'

export default (
  set: AddLocalLlmModelDownloadDrawerSet,
  _get: import('../state').AddLocalLlmModelDownloadDrawerGet,
) =>
  async (providerId: string) => {
    set(s => {
      s.open = true
      s.providerId = providerId
    })
  }

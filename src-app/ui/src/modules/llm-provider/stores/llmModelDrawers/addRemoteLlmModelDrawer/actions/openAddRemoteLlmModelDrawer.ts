import type { AddRemoteLlmModelDrawerSet } from '../state'

export default (
  set: AddRemoteLlmModelDrawerSet,
  _get: import('../state').AddRemoteLlmModelDrawerGet,
) =>
  async (providerId: string, providerType: string) => {
    set(s => {
      s.open = true
      s.providerId = providerId
      s.providerType = providerType
    })
  }

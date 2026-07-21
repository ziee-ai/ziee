import type { AddRemoteLlmModelDrawerSet } from '../state'

export default (
  set: AddRemoteLlmModelDrawerSet,
  _get: import('../state').AddRemoteLlmModelDrawerGet,
) => async () => {
  set(s => {
    s.open = false
    s.loading = false
    s.providerId = null
    s.providerType = null
  })
}

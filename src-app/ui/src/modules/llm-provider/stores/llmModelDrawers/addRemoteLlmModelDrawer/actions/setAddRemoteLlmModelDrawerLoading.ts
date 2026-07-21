import type { AddRemoteLlmModelDrawerSet } from '../state'

export default (
  set: AddRemoteLlmModelDrawerSet,
  _get: import('../state').AddRemoteLlmModelDrawerGet,
) => async (loading: boolean) => {
  set(s => {
    s.loading = loading
  })
}

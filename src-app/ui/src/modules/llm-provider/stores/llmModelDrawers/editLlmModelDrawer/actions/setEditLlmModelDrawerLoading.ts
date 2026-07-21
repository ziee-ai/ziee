import type { EditLlmModelDrawerSet } from '../state'

export default (
  set: EditLlmModelDrawerSet,
  _get: import('../state').EditLlmModelDrawerGet,
) => async (loading: boolean) => {
  set(s => {
    s.loading = loading
  })
}

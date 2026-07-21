import type { EditLlmModelDrawerSet } from '../state'

export default (
  set: EditLlmModelDrawerSet,
  _get: import('../state').EditLlmModelDrawerGet,
) => async () => {
  set(s => {
    s.open = false
    s.loading = false
    s.modelId = null
  })
}

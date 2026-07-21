import type { EditLlmModelDrawerSet } from '../state'

export default (
  set: EditLlmModelDrawerSet,
  _get: import('../state').EditLlmModelDrawerGet,
) => async (modelId: string) => {
  set(s => {
    s.open = true
    s.modelId = modelId
  })
}

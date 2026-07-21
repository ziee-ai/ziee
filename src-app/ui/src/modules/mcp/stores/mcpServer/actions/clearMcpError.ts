import type { McpServerSet } from '../state'

export default (set: McpServerSet) => async () => {
  set(draft => {
    draft.error = null
  })
}

import type { McpUserPolicySet } from '../state'

export default (set: McpUserPolicySet) => () => {
  set(state => {
    state.error = null
  })
}

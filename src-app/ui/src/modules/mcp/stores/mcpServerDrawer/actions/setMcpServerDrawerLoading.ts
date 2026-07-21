import type { McpServerDrawerSet } from '../state'

export default (set: McpServerDrawerSet) => (loading: boolean) =>
  set({ loading })

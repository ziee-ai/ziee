import type { McpServerDrawerSet } from '../state'

export default (set: McpServerDrawerSet) => () =>
  set({
    open: false,
    loading: false,
    editingServer: null,
    prefillData: null,
    isCloning: false,
    mode: 'create',
  })

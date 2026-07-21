import type { McpServersStepSet } from '../state'

export default (set: McpServersStepSet) =>
  async () => {
    set(draft => {
      draft.selectedMcpServerIds = []
      draft.systemServers = []
      draft.hubServers = []
      draft.installedNames = new Set()
      draft.loadingServers = false
      draft.serversError = null
      draft.disabledSystemIds = new Set()
      draft.originalDisabledSystemIds = new Set()
    })
  }

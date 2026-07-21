import type { McpServersStepSet } from '../state'

export default (set: McpServersStepSet) =>
  async (id: string) => {
    set(draft => {
      const idx = draft.selectedMcpServerIds.indexOf(id)
      if (idx >= 0) draft.selectedMcpServerIds.splice(idx, 1)
      else draft.selectedMcpServerIds.push(id)
    })
  }

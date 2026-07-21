import type { McpServersStepSet } from '../state'

export default (set: McpServersStepSet) =>
  async (id: string, enabled: boolean) => {
    set(draft => {
      if (enabled) draft.disabledSystemIds.delete(id)
      else draft.disabledSystemIds.add(id)
    })
  }

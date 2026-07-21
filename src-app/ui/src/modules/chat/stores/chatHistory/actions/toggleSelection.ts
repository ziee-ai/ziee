import type { ChatHistorySet } from '../state'

export default (set: ChatHistorySet) =>
  (id: string) => {
    set(draft => {
      if (draft.selectedIds.has(id)) draft.selectedIds.delete(id)
      else draft.selectedIds.add(id)
    })
  }

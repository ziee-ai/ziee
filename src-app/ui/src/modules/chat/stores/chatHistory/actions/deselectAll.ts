import type { ChatHistorySet } from '../state'

export default (set: ChatHistorySet) =>
  () => {
    set(draft => {
      draft.selectedIds.clear()
    })
  }

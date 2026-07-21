import type { ChatHistorySet } from '../state'

export default (set: ChatHistorySet) =>
  () => {
    set(draft => {
      // The visible list IS the (server-filtered) `conversations` now.
      draft.conversations.forEach(conv => {
        draft.selectedIds.add(conv.id)
      })
    })
  }

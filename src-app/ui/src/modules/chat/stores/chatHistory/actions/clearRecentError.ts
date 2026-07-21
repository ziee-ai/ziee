import type { ChatHistorySet } from '../state'

export default (set: ChatHistorySet) =>
  async () => {
    // Clear a lingering load-MORE error (the widget calls this when the user
    // scrolls away from the failed bottom, so returning retries once instead
    // of a tight loop while pinned at the end).
    set({ recentError: null })
  }

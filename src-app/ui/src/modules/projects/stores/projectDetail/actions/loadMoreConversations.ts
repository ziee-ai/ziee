import { ApiClient } from '@/api-client'
import type { ProjectDetailGet, ProjectDetailSet } from '../state'

export default (set: ProjectDetailSet, get: ProjectDetailGet) =>
  async (projectId: string) => {
    const state = get()
    if (
      !state.conversationsHasMore ||
      state.conversationsLoadingMore ||
      state.conversationsLoading
    ) {
      return
    }
    const nextPage = state.conversationsPage + 1
    try {
      set({ conversationsLoadingMore: true })
      const more = await ApiClient.Project.listConversations({
        id: projectId,
        page: nextPage,
        limit: 20,
      })
      set(draft => {
        // Dedupe by id in case the server returned a row we already have.
        const seen = new Set(draft.conversations.map(c => c.id))
        for (const c of more) if (!seen.has(c.id)) draft.conversations.push(c)
        draft.conversationsPage = nextPage
        draft.conversationsHasMore = more.length === 20
        draft.conversationsLoadingMore = false
      })
    } catch (error) {
      set({
        error:
          error instanceof Error
            ? error.message
            : 'Failed to load more project conversations',
        conversationsLoadingMore: false,
      })
    }
  }

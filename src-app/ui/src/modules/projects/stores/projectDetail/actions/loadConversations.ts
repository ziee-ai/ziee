import { ApiClient } from '@/api-client'
import type { ProjectDetailGet, ProjectDetailSet } from '../state'

/// Page size for the project-conversations list. Matches ChatHistory's primary
/// list; bounded by the backend's PaginationQuery::resolved() clamp (≤100).
const CONVERSATIONS_PAGE_SIZE = 20

export default (set: ProjectDetailSet, _get: ProjectDetailGet) =>
  async (projectId: string) => {
    try {
      set({ conversationsLoading: true, conversationsPage: 1, conversationsError: null })
      const conversations = await ApiClient.Project.listConversations({
        id: projectId,
        page: 1,
        limit: CONVERSATIONS_PAGE_SIZE,
      })
      set({
        conversations,
        conversationsLoading: false,
        conversationsHasMore: conversations.length === CONVERSATIONS_PAGE_SIZE,
      })
    } catch (error) {
      set({
        conversationsError:
          error instanceof Error ? error.message : 'Failed to load project conversations',
        conversationsLoading: false,
      })
    }
  }

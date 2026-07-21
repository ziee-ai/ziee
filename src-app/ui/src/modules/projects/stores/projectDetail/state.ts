import type { ConversationResponse, Project } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const projectDetailState = {
  project: null as Project | null,
  conversations: [] as ConversationResponse[],
  /// Current page (1-based) of `conversations`.
  conversationsPage: 1,
  /// True iff the last page came back full (may be more upstream).
  conversationsHasMore: false,
  // Starts true: the detail page always loads on mount, so the initial render
  // shows the spinner — not the load-failed state — before loadProject runs.
  loading: true,
  conversationsLoading: false,
  /// True while a `loadMoreConversations` request is in flight (distinct from
  /// conversationsLoading so "Load More" spins without re-rendering the list).
  conversationsLoadingMore: false,
  error: null as string | null,
  /// Conversation-list load error, distinct from the shared `error`.
  conversationsError: null as string | null,
}

export type ProjectDetailState = typeof projectDetailState
export type ProjectDetailSet = StoreSet<ProjectDetailState>
export type ProjectDetailGet = () => ProjectDetailState

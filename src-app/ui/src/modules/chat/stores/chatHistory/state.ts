import { enableMapSet } from 'immer'
import type { StoreSet } from '@ziee/framework/store-kit'
import type { ConversationResponse } from '@/api-client/types'

// This store mutates `selectedIds` (a Set) through immer, so the MapSet plugin
// must be enabled. Own it here rather than relying on another store's import
// happening to run first. `enableMapSet` is idempotent.
enableMapSet()

/** Conversation list sort order (mirrors the backend `sort` query param). */
export type ConversationSort = 'recent' | 'oldest' | 'alpha' | 'most_messages'

export const chatHistoryState = {
  conversations: [] as ConversationResponse[],
  recentConversations: [] as ConversationResponse[],
  // Pagination (the /chats history list — search/sort mutable).
  page: 1,
  limit: 20,
  total: 0,
  hasMore: false,
  // Sidebar "recent chats" paging — a DEDICATED, always-unfiltered/recent-sort
  // paging cursor, decoupled from the search/sort-mutable history list above so
  // a /chats reload can never reset the accumulated (infinite-scrolled) sidebar.
  recentPage: 1,
  recentTotal: 0,
  recentHasMore: false,
  recentLoading: false,
  recentLoadingMore: false,
  recentInitialized: false,
  recentError: null as string | null,
  // Monotonic epoch bumped when the recent list is structurally reset (e.g. a
  // delete drains it to empty). An in-flight load captures the epoch and
  // discards its result if it changed underneath — so a stale page-N response
  // can't append onto a list that was reset mid-flight.
  recentLoadSeq: 0,
  // Search + sort state (both applied server-side).
  searchQuery: '',
  sort: 'recent' as ConversationSort,
  // Selection
  selectedIds: new Set<string>(),
  // Loading states
  loading: false,
  loadingMore: false,
  deleting: false,
  // A page-1 refresh (new search/sort) requested while a load was in flight —
  // re-run once the current load settles so the latest query isn't dropped.
  reloadQueued: false,
  error: null as string | null,
  isInitialized: false,
}

export type ChatHistoryState = typeof chatHistoryState
export type ChatHistorySet = StoreSet<ChatHistoryState>
export type ChatHistoryGet = () => ChatHistoryState

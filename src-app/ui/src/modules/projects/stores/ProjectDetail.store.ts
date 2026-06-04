import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  Project,
  ConversationResponse,
} from '@/api-client/types'
import { Stores } from '@/core/stores'

/// Page size for the project-conversations list. Matches what the
/// ChatHistory store uses for its primary list, and is bounded by the
/// backend's PaginationQuery::resolved() clamp (≤100).
const CONVERSATIONS_PAGE_SIZE = 20

// `ProjectFileUploadProgress` + the entire file-related slice
// (files, filesLoading, uploadingFiles, loadFiles, attachFile,
// uploadAndAttachFiles, dismissUploadingFile, detachFile) moved to
// `modules/file/project-extension/stores/ProjectFiles.store.ts` as
// part of the project↔file inversion. Read via `Stores.ProjectFiles.*`.

interface ProjectDetailState {
  project: Project | null
  conversations: ConversationResponse[]
  /// Current page (1-based) of `conversations` — incremented by
  /// `loadMoreConversations` and reset by `loadConversations`.
  conversationsPage: number
  /// True iff the last fetched page came back full
  /// (length == CONVERSATIONS_PAGE_SIZE), so there may be more rows
  /// upstream. Mirrors ChatHistory's heuristic — neither backend
  /// endpoint returns a total count today, so this is the best we
  /// can do without a schema change.
  conversationsHasMore: boolean

  loading: boolean
  conversationsLoading: boolean
  /// True while a `loadMoreConversations` request is in flight.
  /// Distinct from `conversationsLoading` so the "Load More" button
  /// can show a spinner without re-rendering the existing list as
  /// "loading".
  conversationsLoadingMore: boolean

  error: string | null

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void

  loadProject: (projectId: string) => Promise<void>
  loadConversations: (projectId: string) => Promise<void>
  loadMoreConversations: (projectId: string) => Promise<void>
  clearProjectDetailError: () => void
}

export const useProjectDetailStore = create<ProjectDetailState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProjectDetailState => ({
        project: null,
        conversations: [],
        conversationsPage: 1,
        conversationsHasMore: false,
        loading: false,
        conversationsLoading: false,
        conversationsLoadingMore: false,
        error: null,

        __init__: {
          __store__: () => {
            const GROUP = 'ProjectDetailStore'
            const eventBus = Stores.EventBus

            // Refresh the currently-loaded project when it changes upstream.
            eventBus.on(
              'project.updated',
              async event => {
                const current = get().project
                if (current && current.id === event.data.project.id) {
                  set({ project: event.data.project })
                }
              },
              GROUP,
            )

            // F5: drop a conversation from the project's local list
            // when ANY component deletes it (sidebar, chat history
            // page, or this page itself).
            eventBus.on(
              'conversation.deleted',
              async event => {
                set(state => {
                  state.conversations = state.conversations.filter(
                    c => c.id !== event.data.conversationId,
                  )
                })
              },
              GROUP,
            )
          },
        },

        loadProject: async projectId => {
          try {
            set({ loading: true, error: null })
            const project = await ApiClient.Project.get({ id: projectId })
            set({ project, loading: false })
            // File loading is now the file module's responsibility —
            // ProjectFiles.store subscribes to `project.id` changes
            // and reloads automatically.
            void get().loadConversations(projectId)
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load project',
              loading: false,
            })
            throw error
          }
        },

        // Load the first page. Replaces the previous list. Resets
        // conversationsPage to 1 and recomputes conversationsHasMore
        // from the response size.
        loadConversations: async projectId => {
          try {
            set({ conversationsLoading: true, conversationsPage: 1 })
            const conversations = await ApiClient.Project.listConversations({
              id: projectId,
              page: 1,
              limit: CONVERSATIONS_PAGE_SIZE,
            })
            set({
              conversations,
              conversationsLoading: false,
              conversationsHasMore:
                conversations.length === CONVERSATIONS_PAGE_SIZE,
            })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load project conversations',
              conversationsLoading: false,
            })
          }
        },

        // Fetch the NEXT page and append. Guarded against double-call
        // while a previous load is in flight. Sets
        // `conversationsHasMore=false` when the response comes back
        // smaller than the page size (no more rows upstream).
        loadMoreConversations: async projectId => {
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
              limit: CONVERSATIONS_PAGE_SIZE,
            })
            set(draft => {
              // Dedupe by id in case the server returned a row we
              // already have (race with conversation.created on the
              // tail of the prior page).
              const seen = new Set(draft.conversations.map(c => c.id))
              for (const c of more) {
                if (!seen.has(c.id)) draft.conversations.push(c)
              }
              draft.conversationsPage = nextPage
              draft.conversationsHasMore = more.length === CONVERSATIONS_PAGE_SIZE
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
        },

        clearProjectDetailError: () => {
          set({ error: null })
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('ProjectDetailStore')
        },
      }),
    ),
  ),
)

import { ApiClient } from '@/api-client'
import {
  type ConversationResponse,
  Permissions,
  type Project,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/// Page size for the project-conversations list. Matches ChatHistory's primary
/// list; bounded by the backend's PaginationQuery::resolved() clamp (≤100).
const CONVERSATIONS_PAGE_SIZE = 20

// The file-related slice moved to
// `modules/file/project-extension/stores/ProjectFiles.store.ts` as part of the
// project↔file inversion. Read via `Stores.ProjectFiles.*`.

export const ProjectDetail = defineStore('ProjectDetail', {
  immer: true,
  state: {
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
  },
  actions: (set, get) => {
    // Load the first page. Replaces the list, resets page to 1.
    const loadConversations = async (projectId: string) => {
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
    return {
      loadConversations,
      loadProject: async (projectId: string) => {
        try {
          set({ loading: true, error: null })
          const project = await ApiClient.Project.get({ id: projectId })
          set({ project, loading: false })
          // File loading is the file module's responsibility — ProjectFiles
          // subscribes to `project.id` changes and reloads automatically.
          void loadConversations(projectId)
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to load project',
            loading: false,
          })
          throw error
        }
      },
      // Fetch the NEXT page and append. Guarded against double-call.
      loadMoreConversations: async (projectId: string) => {
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
          // Dedupe by id in case the server returned a row we already have.
          const seen = new Set(draft.conversations.map(c => c.id))
          for (const c of more) if (!seen.has(c.id)) draft.conversations.push(c)
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
    }
  },
  init: ({ on, get, set, actions }) => {
    // Refresh the currently-loaded project when it changes upstream.
    on('project.updated', event => {
      const current = get().project
      if (current && current.id === event.data.project.id) {
        set({ project: event.data.project })
      }
    })
    // Cross-device: a remote edit arrives as a `sync:project` frame (the local
    // `project.updated` only fires for same-device mutations). Refetch the open
    // project. Self-gated per the no-403-reconnect convention.
    const reloadOnSync = () => {
      if (!hasPermissionNow(Permissions.ProjectsRead)) return
      const id = get().project?.id
      if (id) void actions.loadProject(id)
    }
    on('sync:project', reloadOnSync)
    on('sync:reconnect', reloadOnSync)
    // Drop a conversation from the list when ANY component deletes it.
    on('conversation.deleted', event => {
      set(state => {
        state.conversations = state.conversations.filter(
          c => c.id !== event.data.conversationId,
        )
      })
    })
    // Detaching a conversation from THIS project drops it from the list.
    on('project.conversation_detached', event => {
      if (event.data.projectId !== get().project?.id) return
      set(state => {
        state.conversations = state.conversations.filter(
          c => c.id !== event.data.conversationId,
        )
      })
    })
  },
})

export const useProjectDetailStore = ProjectDetail.store

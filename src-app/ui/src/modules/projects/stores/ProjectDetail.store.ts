import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  Project,
  File as ProjectFile,
  ConversationResponse,
  UpdateProjectMcpSettingsRequest,
} from '@/api-client/types'
import {
  emitProjectFileAttached,
  emitProjectFileDetached,
  emitProjectUpdated,
} from '@/modules/projects/events'
import { Stores } from '@/core/stores'

/// Page size for the project-conversations list. Matches what the
/// ChatHistory store uses for its primary list, and is bounded by the
/// backend's PaginationQuery::resolved() clamp (≤100).
const CONVERSATIONS_PAGE_SIZE = 20

interface ProjectDetailState {
  project: Project | null
  files: ProjectFile[]
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
  filesLoading: boolean
  conversationsLoading: boolean
  /// True while a `loadMoreConversations` request is in flight.
  /// Distinct from `conversationsLoading` so the "Load More" button
  /// can show a spinner without re-rendering the existing list as
  /// "loading".
  conversationsLoadingMore: boolean
  attaching: boolean
  detaching: boolean

  error: string | null

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void

  loadProject: (projectId: string) => Promise<void>
  loadFiles: (projectId: string) => Promise<void>
  loadConversations: (projectId: string) => Promise<void>
  loadMoreConversations: (projectId: string) => Promise<void>
  attachFile: (projectId: string, fileId: string) => Promise<void>
  detachFile: (projectId: string, fileId: string) => Promise<void>
  updateMcpSettings: (
    projectId: string,
    settings: UpdateProjectMcpSettingsRequest,
  ) => Promise<Project>
  clearProjectDetailError: () => void
}

export const useProjectDetailStore = create<ProjectDetailState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProjectDetailState => ({
        project: null,
        files: [],
        conversations: [],
        conversationsPage: 1,
        conversationsHasMore: false,
        loading: false,
        filesLoading: false,
        conversationsLoading: false,
        conversationsLoadingMore: false,
        attaching: false,
        detaching: false,
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

            // Refresh the file list when something attaches/detaches.
            eventBus.on(
              'project.file_attached',
              async event => {
                const current = get().project
                if (current && current.id === event.data.projectId) {
                  await get().loadFiles(current.id)
                }
              },
              GROUP,
            )

            eventBus.on(
              'project.file_detached',
              async event => {
                const current = get().project
                if (current && current.id === event.data.projectId) {
                  set(state => {
                    state.files = state.files.filter(
                      f => f.id !== event.data.fileId,
                    )
                  })
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
            // Fan-out the satellite fetches in parallel; failures bubble
            // up through their own loading flags.
            void get().loadFiles(projectId)
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

        loadFiles: async projectId => {
          try {
            set({ filesLoading: true })
            const response = await ApiClient.Project.listFiles({
              id: projectId,
            })
            set({ files: response.files, filesLoading: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load project files',
              filesLoading: false,
            })
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

        attachFile: async (projectId, fileId) => {
          try {
            set({ attaching: true, error: null })
            await ApiClient.Project.attachFile({ id: projectId, file_id: fileId })
            await emitProjectFileAttached(projectId, fileId)
            set({ attaching: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to attach file',
              attaching: false,
            })
            throw error
          }
        },

        detachFile: async (projectId, fileId) => {
          try {
            set({ detaching: true, error: null })
            await ApiClient.Project.detachFile({
              id: projectId,
              file_id: fileId,
            })
            await emitProjectFileDetached(projectId, fileId)
            set({ detaching: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to detach file',
              detaching: false,
            })
            throw error
          }
        },

        updateMcpSettings: async (projectId, settings) => {
          try {
            const updated = await ApiClient.Project.updateMcpSettings({
              id: projectId,
              ...settings,
            })
            await emitProjectUpdated(updated)
            set({ project: updated })
            return updated
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to update MCP settings',
            })
            throw error
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

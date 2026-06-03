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

/**
 * Per-file upload progress tracked while a project knowledge upload
 * is in flight. The map key is a synthetic local id (so the same file
 * uploaded twice gets two separate progress rows). Mirrors the chat
 * FileStore's pattern but scoped to the currently-loaded project.
 */
export interface ProjectFileUploadProgress {
  id: string
  filename: string
  size: number
  progress: number
  status: 'pending' | 'uploading' | 'error'
  error?: string
}

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

  /// In-flight project-knowledge uploads keyed by synthetic local id.
  /// Done entries are removed on success (the new File appears in
  /// `files` via the `project.file_attached` event); failed entries
  /// remain with `status: 'error'` until the user clears them.
  uploadingFiles: Map<string, ProjectFileUploadProgress>

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
  /// Upload N new files via multipart and attach to the project in
  /// one shot (POST /api/projects/{id}/files/upload). Each file gets
  /// its own progress row; errors stay visible until cleared.
  uploadAndAttachFiles: (projectId: string, files: File[]) => Promise<void>
  /// Drop a finished/errored upload row from `uploadingFiles`.
  dismissUploadingFile: (uploadId: string) => void
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
        uploadingFiles: new Map(),
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

        uploadAndAttachFiles: async (projectId, files) => {
          // Per-file upload — each gets its own progress row + own
          // event emit on success. Failures stay in `uploadingFiles`
          // with `status: 'error'` so the user can read the message
          // before dismissing; subsequent uploads in the same batch
          // proceed independently. After ALL uploads in this batch
          // settle we re-fetch the files list once (the event-bus
          // listener does the same job, but doing it directly here
          // avoids relying on cross-listener timing — fixes the
          // "uploaded many files but no files are shown" race when
          // multiple emits arrive faster than loadFiles can settle).
          let anySucceeded = false
          await Promise.all(
            files.map(async file => {
              const uploadId = `up_${Date.now()}_${Math.random()
                .toString(36)
                .slice(2, 11)}`
              set(state => {
                state.uploadingFiles.set(uploadId, {
                  id: uploadId,
                  filename: file.name,
                  size: file.size,
                  progress: 0,
                  status: 'pending',
                })
              })

              try {
                set(state => {
                  const entry = state.uploadingFiles.get(uploadId)
                  if (entry) entry.status = 'uploading'
                })

                // Path parameter `{id}` is read from FormData entries
                // by the api-client when params is FormData — see
                // core.ts's `isFormData` branch. So append BOTH the
                // path-id AND the multipart file field.
                const formData = new FormData()
                formData.append('id', projectId)
                formData.append('file', file)

                const uploaded = await ApiClient.Project.uploadAndAttachFile(
                  formData as unknown as { id: string } & FormData,
                  {
                    fileUploadProgress: {
                      onProgress: progress => {
                        set(state => {
                          const entry = state.uploadingFiles.get(uploadId)
                          if (entry) entry.progress = progress
                        })
                      },
                    },
                  },
                )

                // Success: drop the progress row + flag for the
                // post-batch reload. emitProjectFileAttached lets
                // OTHER subscribers (e.g. the inline preview chip)
                // react; the local reload below handles our own
                // files list deterministically.
                set(state => {
                  state.uploadingFiles.delete(uploadId)
                })
                anySucceeded = true
                await emitProjectFileAttached(projectId, uploaded.id)
              } catch (error) {
                set(state => {
                  const entry = state.uploadingFiles.get(uploadId)
                  if (entry) {
                    entry.status = 'error'
                    entry.error =
                      error instanceof Error
                        ? error.message
                        : 'Upload failed'
                  }
                })
              }
            }),
          )
          // One reload after the batch. The event-bus listener does
          // the same job for OTHER project-detail subscribers, but
          // doing it here directly is what makes our own files list
          // reflect the upload deterministically (a stack of N
          // listener callbacks all calling loadFiles in parallel can
          // race against each other and against subsequent state
          // writes).
          if (anySucceeded) {
            await get().loadFiles(projectId)
          }
        },

        dismissUploadingFile: uploadId => {
          set(state => {
            state.uploadingFiles.delete(uploadId)
          })
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

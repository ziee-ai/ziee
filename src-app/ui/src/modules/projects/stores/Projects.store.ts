import { enableMapSet } from 'immer'
import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  type ConversationResponse,
  type CreateProjectRequest,
  Permissions,
  type Project,
  type UpdateProjectRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import {
  emitProjectConversationAttached,
  emitProjectConversationDetached,
  emitProjectCreated,
  emitProjectDeleted,
  emitProjectUpdated,
} from '@/modules/projects/events'

enableMapSet()

interface ProjectsState {
  projects: Map<string, Project>
  isInitialized: boolean

  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean
  duplicating: boolean

  error: string | null

  __init__: {
    __store__?: () => void
    projects: () => Promise<void>
  }
  __destroy__?: () => void

  loadProjects: (force?: boolean) => Promise<void>
  createProject: (data: CreateProjectRequest) => Promise<Project>
  updateProject: (id: string, data: UpdateProjectRequest) => Promise<Project>
  deleteProject: (id: string) => Promise<void>
  duplicateProject: (id: string) => Promise<Project | undefined>
  /**
   * Attach (or re-attach across projects) a conversation to a
   * project. Idempotent — re-calling refreshes the project MCP
   * snapshot stored on the conversation. Emits
   * `project.conversation_attached` on success.
   */
  attachConversation: (
    projectId: string,
    conversationId: string,
  ) => Promise<ConversationResponse>
  /**
   * Detach a conversation from a project ("unfile"). Clears the
   * per-conversation MCP snapshot row. Emits
   * `project.conversation_detached` on success.
   */
  detachConversation: (
    projectId: string,
    conversationId: string,
  ) => Promise<void>
  clearProjectsError: () => void
}

export const useProjectsStore = create<ProjectsState>()(
  subscribeWithSelector(
    immer(
      (set, get): ProjectsState => ({
        projects: new Map<string, Project>(),
        isInitialized: false,
        loading: false,
        creating: false,
        updating: false,
        deleting: false,
        duplicating: false,
        error: null,

        __init__: {
          __store__: () => {
            const GROUP = 'ProjectsStore'
            const eventBus = Stores.EventBus

            eventBus.on(
              'project.created',
              async event => {
                set(state => {
                  state.projects.set(event.data.project.id, event.data.project)
                })
              },
              GROUP,
            )

            eventBus.on(
              'project.updated',
              async event => {
                set(state => {
                  state.projects.set(event.data.project.id, event.data.project)
                })
              },
              GROUP,
            )

            eventBus.on(
              'project.deleted',
              async event => {
                set(state => {
                  state.projects.delete(event.data.projectId)
                })
              },
              GROUP,
            )

            // Cross-device sync: a `project` change on another device (or a
            // reconnect resync) triggers a full reload. `loadProjects`
            // self-gates on `projects::read` so a non-admin reconnect never
            // hits `GET /api/projects` → 403.
            const reload = () => void get().loadProjects(true)
            eventBus.on('sync:project', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
          projects: () => get().loadProjects(),
        },

        loadProjects: async (force = false) => {
          if (!hasPermissionNow(Permissions.ProjectsRead)) return
          const state = get()
          if ((state.isInitialized && !force) || state.loading) return
          try {
            set({ loading: true, error: null })
            const response = await ApiClient.Project.list({
              page: 1,
              limit: 50,
            })
            set({
              projects: new Map(
                (response?.projects ?? []).map((p: Project) => [p.id, p]),
              ),
              isInitialized: true,
              loading: false,
            })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to load projects',
              loading: false,
            })
            throw error
          }
        },

        createProject: async data => {
          try {
            set({ creating: true, error: null })
            const project = await ApiClient.Project.create(data)
            await emitProjectCreated(project)
            set({ creating: false })
            return project
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to create project',
              creating: false,
            })
            throw error
          }
        },

        updateProject: async (id, data) => {
          try {
            set({ updating: true, error: null })
            const project = await ApiClient.Project.update({ id, ...data })
            await emitProjectUpdated(project)
            set({ updating: false })
            return project
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to update project',
              updating: false,
            })
            throw error
          }
        },

        deleteProject: async id => {
          // Idempotency guard: a double-click on Delete used to fire
          // two DELETE requests, the 2nd would 404 (race), surfacing a
          // confusing "failed to delete" toast after the 1st succeeded.
          // Bail out if a delete is already in flight.
          if (get().deleting) return
          try {
            set({ deleting: true, error: null })
            await ApiClient.Project.delete({ id })
            await emitProjectDeleted(id)
            set({ deleting: false })
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to delete project',
              deleting: false,
            })
            throw error
          }
        },

        duplicateProject: async id => {
          // Idempotency guard: a double-click on Duplicate used to
          // create TWO "(copy)" projects via two POSTs racing the
          // unique-name disambiguation loop. Single-flight per store.
          //
          // Returns `undefined` on the already-in-flight branch (vs
          // throwing) so callers don't surface a confusing "Failed to
          // duplicate" toast when the FIRST call is still running and
          // will succeed. Matches the silent-return semantic of
          // `deleteProject` for consistency. The caller's promise
          // resolves; if they want to surface "duplicated as X" they
          // must check the return is defined.
          if (get().duplicating) return undefined
          try {
            set({ duplicating: true, error: null })
            const project = await ApiClient.Project.duplicate({ id })
            await emitProjectCreated(project)
            set({ duplicating: false })
            return project
          } catch (error) {
            set({
              error:
                error instanceof Error
                  ? error.message
                  : 'Failed to duplicate project',
              duplicating: false,
            })
            throw error
          }
        },

        attachConversation: async (projectId, conversationId) => {
          // Query the conversation's current project BEFORE updating,
          // so the event carries the correct `fromProjectId` (needed by
          // listeners that track per-conversation project membership).
          const currentProject = await ApiClient.Project.forConversation({
            conversation_id: conversationId,
          })
          const fromProjectId = currentProject?.id ?? null

          // API call + event only. The chat extension at
          // `projects/chat-extension/` subscribes to the emitted
          // event and patches chat-side state — keeps this module
          // free of chat-store calls.
          const response = await ApiClient.Project.attachConversation({
            id: projectId,
            conversation_id: conversationId,
          })
          await emitProjectConversationAttached(
            projectId,
            conversationId,
            fromProjectId,
          )
          return response
        },

        detachConversation: async (projectId, conversationId) => {
          await ApiClient.Project.detachConversation({
            id: projectId,
            conversation_id: conversationId,
          })
          await emitProjectConversationDetached(projectId, conversationId)
        },

        clearProjectsError: () => {
          set({ error: null })
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('ProjectsStore')
        },
      }),
    ),
  ),
)

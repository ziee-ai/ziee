import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import {
  type ConversationResponse,
  type CreateProjectRequest,
  Permissions,
  type Project,
  type UpdateProjectRequest,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import {
  emitProjectConversationAttached,
  emitProjectConversationDetached,
  emitProjectCreated,
  emitProjectDeleted,
  emitProjectUpdated,
} from '@/modules/projects/events'

enableMapSet()

export const Projects = defineStore('Projects', {
  immer: true,
  state: {
    projects: new Map<string, Project>(),
    isInitialized: false,
    loading: false,
    creating: false,
    updating: false,
    deleting: false,
    duplicating: false,
    error: null as string | null,
  },
  actions: (set, get) => ({
    loadProjects: async (force = false) => {
      if (!hasPermissionNow(Permissions.ProjectsRead)) return
      const state = get()
      if ((state.isInitialized && !force) || state.loading) return
      try {
        set({ loading: true, error: null })
        const response = await ApiClient.Project.list({ page: 1, limit: 50 })
        set({
          projects: new Map((response?.projects ?? []).map((p: Project) => [p.id, p])),
          isInitialized: true,
          loading: false,
        })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to load projects',
          loading: false,
        })
        throw error
      }
    },
    createProject: async (data: CreateProjectRequest): Promise<Project> => {
      try {
        set({ creating: true, error: null })
        const project = await ApiClient.Project.create(data)
        await emitProjectCreated(project)
        set({ creating: false })
        return project
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to create project',
          creating: false,
        })
        throw error
      }
    },
    updateProject: async (id: string, data: UpdateProjectRequest): Promise<Project> => {
      try {
        set({ updating: true, error: null })
        const project = await ApiClient.Project.update({ id, ...data })
        await emitProjectUpdated(project)
        set({ updating: false })
        return project
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to update project',
          updating: false,
        })
        throw error
      }
    },
    deleteProject: async (id: string): Promise<void> => {
      // Idempotency guard: bail if a delete is already in flight (a double-click
      // used to fire two DELETEs, the 2nd 404ing after the 1st succeeded).
      if (get().deleting) return
      try {
        set({ deleting: true, error: null })
        await ApiClient.Project.delete({ id })
        await emitProjectDeleted(id)
        set({ deleting: false })
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to delete project',
          deleting: false,
        })
        throw error
      }
    },
    duplicateProject: async (id: string): Promise<Project | undefined> => {
      // Single-flight per store. Returns `undefined` on the already-in-flight
      // branch (vs throwing) so callers don't surface a confusing toast while
      // the FIRST call is still running. Matches deleteProject's semantics.
      if (get().duplicating) return undefined
      try {
        set({ duplicating: true, error: null })
        const project = await ApiClient.Project.duplicate({ id })
        await emitProjectCreated(project)
        set({ duplicating: false })
        return project
      } catch (error) {
        set({
          error: error instanceof Error ? error.message : 'Failed to duplicate project',
          duplicating: false,
        })
        throw error
      }
    },
    /**
     * Attach (or re-attach across projects) a conversation to a project.
     * Idempotent — re-calling refreshes the project MCP snapshot. Emits
     * `project.conversation_attached` on success.
     */
    attachConversation: async (
      projectId: string,
      conversationId: string,
    ): Promise<ConversationResponse> => {
      // Query the conversation's current project BEFORE updating so the event
      // carries the correct `fromProjectId`.
      const currentProject = await ApiClient.Project.forConversation({
        conversation_id: conversationId,
      })
      const fromProjectId = currentProject?.id ?? null
      // API call + event only. The chat extension patches chat-side state.
      const response = await ApiClient.Project.attachConversation({
        id: projectId,
        conversation_id: conversationId,
      })
      await emitProjectConversationAttached(projectId, conversationId, fromProjectId)
      return response
    },
    /**
     * Detach a conversation from a project ("unfile"). Emits
     * `project.conversation_detached` on success.
     */
    detachConversation: async (projectId: string, conversationId: string): Promise<void> => {
      await ApiClient.Project.detachConversation({
        id: projectId,
        conversation_id: conversationId,
      })
      await emitProjectConversationDetached(projectId, conversationId)
    },
    clearProjectsError: () => {
      set({ error: null })
    },
  }),
  init: ({ on, set, actions }) => {
    on('project.created', event => {
      set(state => {
        state.projects.set(event.data.project.id, event.data.project)
      })
    })
    on('project.updated', event => {
      set(state => {
        state.projects.set(event.data.project.id, event.data.project)
      })
    })
    on('project.deleted', event => {
      set(state => {
        state.projects.delete(event.data.projectId)
      })
    })
    // Cross-device sync: a `project` change on another device (or reconnect
    // resync) triggers a full reload. loadProjects self-gates on projects::read.
    const reload = () => void actions.loadProjects(true)
    on('sync:project', reload)
    on('sync:reconnect', reload)
    void actions.loadProjects()
  },
})

export const useProjectsStore = Projects.store

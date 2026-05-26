import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import type {
  Project,
  CreateProjectRequest,
  UpdateProjectRequest,
} from '@/api-client/types'
import {
  emitProjectCreated,
  emitProjectUpdated,
  emitProjectDeleted,
} from '@/modules/projects/events'
import { Stores } from '@/core/stores'

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

  loadProjects: () => Promise<void>
  createProject: (data: CreateProjectRequest) => Promise<Project>
  updateProject: (id: string, data: UpdateProjectRequest) => Promise<Project>
  deleteProject: (id: string) => Promise<void>
  duplicateProject: (id: string) => Promise<Project | undefined>
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
          },
          projects: () => get().loadProjects(),
        },

        loadProjects: async () => {
          const state = get()
          if (state.isInitialized || state.loading) return
          try {
            set({ loading: true, error: null })
            const response = await ApiClient.Project.list({
              page: 1,
              limit: 50,
            })
            set({
              projects: new Map(
                response.projects.map((p: Project) => [p.id, p]),
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

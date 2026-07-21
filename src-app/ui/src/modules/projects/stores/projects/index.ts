import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { projectsState, type ProjectsState } from './state'
import type { Actions } from './actions.gen'

const ProjectsDef = defineStore<ProjectsState, Actions>('Projects', {
  immer: true,
  state: projectsState,
  actions: import.meta.glob('./actions/*.ts'),
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

export const Projects = registerLazyStore(ProjectsDef)
export const useProjectsStore = ProjectsDef.store

import { projectDrawerState, type ProjectDrawerState } from './state'
import type { Actions } from './actions.gen'

import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'

const ProjectDrawerDef = defineStore<ProjectDrawerState, Actions>('ProjectDrawer', {
  immer: true,
  state: projectDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    on('project.updated', event => {
      if (get().editingProject?.id === event.data.project.id) {
        set({ editingProject: event.data.project })
      }
    })
    on('project.deleted', event => {
      if (get().editingProject?.id === event.data.projectId) actions.closeProjectDrawer()
    })
  },
})
export const ProjectDrawer = registerLazyStore(ProjectDrawerDef)
export const useProjectDrawerStore = ProjectDrawerDef.store

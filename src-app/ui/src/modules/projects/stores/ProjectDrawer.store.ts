import type { Project } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const ProjectDrawer = defineStore('ProjectDrawer', {
  state: {
    open: false,
    loading: false,
    editingProject: null as Project | null,
  },
  actions: set => ({
    openProjectDrawer: (project: Project | null = null) =>
      set({ open: true, editingProject: project, loading: false }),
    closeProjectDrawer: () => set({ open: false, loading: false, editingProject: null }),
    setProjectDrawerLoading: (loading: boolean) => set({ loading }),
  }),
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

export const useProjectDrawerStore = ProjectDrawer.store

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Project } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface ProjectDrawerState {
  open: boolean
  loading: boolean
  editingProject: Project | null

  openProjectDrawer: (project?: Project | null) => void
  closeProjectDrawer: () => void
  setProjectDrawerLoading: (loading: boolean) => void

  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useProjectDrawerStore = create<ProjectDrawerState>()(
  subscribeWithSelector(
    (set, get): ProjectDrawerState => ({
      open: false,
      loading: false,
      editingProject: null,

      __init__: {
        __store__: () => {
          const GROUP = 'ProjectDrawerStore'
          const eventBus = Stores.EventBus

          eventBus.on(
            'project.updated',
            async event => {
              const state = get()
              if (state.editingProject?.id === event.data.project.id) {
                set({ editingProject: event.data.project })
              }
            },
            GROUP,
          )

          eventBus.on(
            'project.deleted',
            async event => {
              const state = get()
              if (state.editingProject?.id === event.data.projectId) {
                get().closeProjectDrawer()
              }
            },
            GROUP,
          )
        },
      },

      openProjectDrawer: (project = null) => {
        set({ open: true, editingProject: project })
      },

      closeProjectDrawer: () => {
        set({ open: false, loading: false, editingProject: null })
      },

      setProjectDrawerLoading: loading => {
        set({ loading })
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('ProjectDrawerStore')
      },
    }),
  ),
)

import type { Workflow } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const WorkflowDrawer = defineStore('WorkflowDrawer', {
  immer: true,
  state: { isOpen: false, workflow: null as Workflow | null },
  actions: set => ({
    open: (workflow: Workflow) =>
      set(d => {
        d.isOpen = true
        d.workflow = workflow
      }),
    close: () =>
      set(d => {
        d.isOpen = false
      }),
  }),
})

export const useWorkflowDrawerStore = WorkflowDrawer.store

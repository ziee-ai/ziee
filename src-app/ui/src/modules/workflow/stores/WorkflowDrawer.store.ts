import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import type { Workflow } from '@/api-client/types'

interface WorkflowDrawerState {
  isOpen: boolean
  workflow: Workflow | null
  open: (workflow: Workflow) => void
  close: () => void
}

export const useWorkflowDrawerStore = create<WorkflowDrawerState>()(
  subscribeWithSelector(
    immer(
      (set): WorkflowDrawerState => ({
        isOpen: false,
        workflow: null,
        open: (workflow: Workflow) =>
          set(draft => {
            draft.isOpen = true
            draft.workflow = workflow
          }),
        close: () =>
          set(draft => {
            draft.isOpen = false
          }),
      }),
    ),
  ),
)

import type { Project } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const projectDrawerState = {
  open: false,
  loading: false,
  editingProject: null as Project | null,
}

export type ProjectDrawerState = typeof projectDrawerState
export type ProjectDrawerSet = StoreSet<ProjectDrawerState>
export type ProjectDrawerGet = () => ProjectDrawerState

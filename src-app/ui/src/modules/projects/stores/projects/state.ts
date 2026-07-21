import type { StoreSet } from '@ziee/framework/store-kit'
import type { Project } from '@/api-client/types'

export const projectsState = {
  projects: new Map<string, Project>(),
  isInitialized: false,
  loading: false,
  creating: false,
  updating: false,
  deleting: false,
  duplicating: false,
  error: null as string | null,
}

export type ProjectsState = typeof projectsState
export type ProjectsSet = StoreSet<ProjectsState>
export type ProjectsGet = () => ProjectsState

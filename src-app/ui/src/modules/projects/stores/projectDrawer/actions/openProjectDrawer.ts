import type { Project } from '@/api-client/types'
import type { ProjectDrawerGet, ProjectDrawerSet } from '../state'

export default (set: ProjectDrawerSet, _get: ProjectDrawerGet) =>
  async (project: Project | null = null) => {
    set({ open: true, editingProject: project, loading: false })
  }

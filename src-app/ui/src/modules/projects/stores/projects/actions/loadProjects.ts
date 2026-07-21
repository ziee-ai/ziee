import { ApiClient } from '@/api-client'
import type { Project } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { ProjectsGet, ProjectsSet } from '../state'

export default (set: ProjectsSet, get: ProjectsGet) =>
  async (force = false) => {
    if (!hasPermissionNow(Permissions.ProjectsRead)) return
    const state = get()
    if ((state.isInitialized && !force) || state.loading) return
    try {
      set({ loading: true, error: null })
      const response = await ApiClient.Project.list({ page: 1, limit: 50 })
      set({
        projects: new Map((response?.projects ?? []).map((p: Project) => [p.id, p])),
        isInitialized: true,
        loading: false,
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load projects',
        loading: false,
      })
      throw error
    }
  }

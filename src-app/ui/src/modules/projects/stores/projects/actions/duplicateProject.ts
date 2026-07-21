import { ApiClient } from '@/api-client'
import type { Project } from '@/api-client/types'
import type { ProjectsGet, ProjectsSet } from '../state'
import { emitProjectCreated } from '@/modules/projects/events'

export default (set: ProjectsSet, get: ProjectsGet) =>
  async (id: string): Promise<Project | undefined> => {
    // Single-flight per store. Returns `undefined` on the already-in-flight
    // branch (vs throwing) so callers don't surface a confusing toast while
    // the FIRST call is still running. Matches deleteProject's semantics.
    if (get().duplicating) return undefined
    try {
      set({ duplicating: true, error: null })
      const project = await ApiClient.Project.duplicate({ id })
      await emitProjectCreated(project)
      set({ duplicating: false })
      return project
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to duplicate project',
        duplicating: false,
      })
      throw error
    }
  }

import { ApiClient } from '@/api-client'
import type { ProjectsGet, ProjectsSet } from '../state'
import { emitProjectDeleted } from '@/modules/projects/events'

export default (set: ProjectsSet, get: ProjectsGet) =>
  async (id: string): Promise<void> => {
    // Idempotency guard: bail if a delete is already in flight (a double-click
    // used to fire two DELETEs, the 2nd 404ing after the 1st succeeded).
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.Project.delete({ id })
      await emitProjectDeleted(id)
      set({ deleting: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete project',
        deleting: false,
      })
      throw error
    }
  }

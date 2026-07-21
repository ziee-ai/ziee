import { ApiClient } from '@/api-client'
import type { UpdateProjectRequest, Project } from '@/api-client/types'
import type { ProjectsGet, ProjectsSet } from '../state'
import { emitProjectUpdated } from '@/modules/projects/events'

export default (set: ProjectsSet, _get: ProjectsGet) =>
  async (id: string, data: UpdateProjectRequest): Promise<Project> => {
    try {
      set({ updating: true, error: null })
      const project = await ApiClient.Project.update({ id, ...data })
      await emitProjectUpdated(project)
      set({ updating: false })
      return project
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to update project',
        updating: false,
      })
      throw error
    }
  }

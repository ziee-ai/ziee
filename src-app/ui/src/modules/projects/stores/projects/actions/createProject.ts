import { ApiClient } from '@/api-client'
import type { CreateProjectRequest, Project } from '@/api-client/types'
import type { ProjectsGet, ProjectsSet } from '../state'
import { emitProjectCreated } from '@/modules/projects/events'

export default (set: ProjectsSet, _get: ProjectsGet) =>
  async (data: CreateProjectRequest): Promise<Project> => {
    try {
      set({ creating: true, error: null })
      const project = await ApiClient.Project.create(data)
      await emitProjectCreated(project)
      set({ creating: false })
      return project
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to create project',
        creating: false,
      })
      throw error
    }
  }

import { ApiClient } from '@/api-client'
import type { ProjectDetailGet, ProjectDetailSet } from '../state'
import loadConversationsFactory from './loadConversations'

export default (set: ProjectDetailSet, get: ProjectDetailGet) => {
  const loadConversations = loadConversationsFactory(set, get)
  return async (projectId: string) => {
    try {
      set({ loading: true, error: null })
      const project = await ApiClient.Project.get({ id: projectId })
      set({ project, loading: false })
      // File loading is the file module's responsibility — ProjectFiles
      // subscribes to `project.id` changes and reloads automatically.
      void loadConversations(projectId)
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load project',
        loading: false,
      })
      throw error
    }
  }
}

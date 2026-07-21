import { ApiClient } from '@/api-client'
import type { ProjectMcpSettingsGet, ProjectMcpSettingsSet } from '../state'

export default (set: ProjectMcpSettingsSet, _get: ProjectMcpSettingsGet) =>
  async (projectId: string) => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const settings = await ApiClient.Project.getMcpSettings({ id: projectId })
      set(s => {
        s.settings = settings
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load MCP settings'
        s.loading = false
      })
    }
  }

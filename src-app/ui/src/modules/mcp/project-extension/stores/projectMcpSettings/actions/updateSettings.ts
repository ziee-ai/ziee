import { ApiClient } from '@/api-client'
import type {
  ProjectMcpSettingsRequest,
  ProjectMcpSettingsResponse,
} from '@/api-client/types'
import { Stores } from '@ziee/framework/stores'
import type { ProjectMcpSettingsGet, ProjectMcpSettingsSet } from '../state'

export default (set: ProjectMcpSettingsSet, _get: ProjectMcpSettingsGet) =>
  async (
    projectId: string,
    payload: ProjectMcpSettingsRequest,
  ): Promise<ProjectMcpSettingsResponse> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const updated = await ApiClient.Project.updateMcpSettings({
        id: projectId,
        ...payload,
      })
      set(s => {
        s.settings = updated
        s.saving = false
      })
      await Stores.EventBus.emit({
        type: 'project.mcp_updated',
        data: { projectId },
      })
      return updated
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to update MCP settings'
        s.saving = false
      })
      throw error
    }
  }

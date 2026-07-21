import { ApiClient } from '@/api-client'
import type { ProjectHostMountsSet } from '../state'

export default (set: ProjectHostMountsSet) =>
  async (projectId: string) => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const body = await ApiClient.HostMount.getProjectMounts({ project_id: projectId })
      set(s => {
        s.mounts = body.mounts
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to load mounts'
        s.loading = false
      })
    }
  }

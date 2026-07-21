import { ApiClient } from '@/api-client'
import type { MountEntry } from '@/api-client/types'
import type { ProjectHostMountsSet } from '../state'

export default (set: ProjectHostMountsSet) =>
  async (projectId: string, mounts: MountEntry[]) => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const body = await ApiClient.HostMount.putProjectMounts({ project_id: projectId, mounts })
      set(s => {
        s.mounts = body.mounts
        s.saving = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to save mounts'
        s.saving = false
      })
      throw error
    }
  }

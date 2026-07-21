import { ApiClient } from '@/api-client'
import type { CitationsGet, CitationsSet } from '../state'

export default (set: CitationsSet, _get: CitationsGet) =>
  async (projectId?: string | null) => {
    const pid = projectId !== undefined ? projectId : _get().projectId
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const resp = await ApiClient.Citations.list(pid ? { project_id: pid } : {})
      set(s => {
        s.entries = resp.entries
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load citations'
        s.loading = false
      })
    }
  }

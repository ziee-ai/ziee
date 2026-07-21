import { ApiClient } from '@/api-client'
import type { MemoryAdminGet, MemoryAdminSet } from '../state'

export default (set: MemoryAdminSet, _get: MemoryAdminGet) => async () => {
  try {
    const status = await ApiClient.MemoryAdmin.ftsRebuildStatus()
    set(s => {
      s.ftsRebuildStatus = status
    })
  } catch {
    // Same rationale as loadRebuildStatus.
  }
}

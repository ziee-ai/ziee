import { ApiClient } from '@/api-client'
import type { MemoryAdminGet, MemoryAdminSet } from '../state'

export default (set: MemoryAdminSet, _get: MemoryAdminGet) => async () => {
  try {
    const status = await ApiClient.MemoryAdmin.rebuildStatus()
    set(s => {
      s.rebuildStatus = status
    })
  } catch {
    // Polling failure shouldn't surface as an error toast.
  }
}

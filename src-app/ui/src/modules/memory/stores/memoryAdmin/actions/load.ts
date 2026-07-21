import { ApiClient } from '@/api-client'
import type { MemoryAdminGet, MemoryAdminSet } from '../state'

export default (set: MemoryAdminSet, _get: MemoryAdminGet) => async () => {
  set(s => {
    s.loading = true
    s.error = null
  })
  try {
    const row = await ApiClient.MemoryAdmin.get()
    set(s => {
      s.settings = row
      s.loading = false
    })
  } catch (error) {
    set(s => {
      s.error = error instanceof Error ? error.message : 'Failed to load admin settings'
      s.loading = false
    })
  }
}

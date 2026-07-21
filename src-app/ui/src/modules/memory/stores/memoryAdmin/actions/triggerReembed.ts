import { ApiClient } from '@/api-client'
import type { MemoryAdminGet, MemoryAdminSet } from '../state'

export default (set: MemoryAdminSet, _get: MemoryAdminGet) => async () => {
  set(s => {
    s.triggeringReembed = true
    s.error = null
  })
  try {
    await ApiClient.MemoryAdmin.reembed()
    set(s => {
      s.triggeringReembed = false
    })
  } catch (error) {
    set(s => {
      s.error = error instanceof Error ? error.message : 'Trigger failed'
      s.triggeringReembed = false
    })
    throw error
  }
}

import { ApiClient } from '@/api-client'
import type { MemoryAuditGet, MemoryAuditSet } from '../state'

export default (set: MemoryAuditSet, _get: MemoryAuditGet) =>
  async (limit: number) => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const rows = await ApiClient.MemoryAudit.list({ limit })
      set(s => {
        s.entries = rows
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to load audit entries'
        s.loading = false
      })
    }
  }

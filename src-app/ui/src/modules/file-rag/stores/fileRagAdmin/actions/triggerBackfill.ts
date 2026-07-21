import { ApiClient } from '@/api-client'
import type { FileRagAdminSet } from '../state'

export default (set: FileRagAdminSet) =>
  async (): Promise<void> => {
    set(s => {
      s.triggeringBackfill = true
      s.error = null
    })
    try {
      await ApiClient.FileRagAdmin.backfill()
      set(s => {
        s.triggeringBackfill = false
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Trigger failed'
        s.triggeringBackfill = false
      })
      throw error
    }
  }

import { ApiClient } from '@/api-client'
import type { MemoryAdminGet, MemoryAdminSet } from '../state'

export default (set: MemoryAdminSet, _get: MemoryAdminGet) =>
  async (dictionary: string) => {
    set(s => {
      s.triggeringFtsRebuild = true
      s.error = null
    })
    try {
      await ApiClient.MemoryAdmin.ftsRebuild({ dictionary })
      set(s => {
        s.triggeringFtsRebuild = false
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Trigger failed'
        s.triggeringFtsRebuild = false
      })
      throw error
    }
  }

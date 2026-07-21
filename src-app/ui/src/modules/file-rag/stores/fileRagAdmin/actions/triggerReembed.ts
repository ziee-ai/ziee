import { ApiClient } from '@/api-client'
import type { FileRagAdminSet } from '../state'

export default (set: FileRagAdminSet) =>
  async (): Promise<void> => {
    set(s => {
      s.triggeringReembed = true
      s.error = null
    })
    try {
      await ApiClient.FileRagAdmin.reembed()
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

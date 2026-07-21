import { ApiClient } from '@/api-client'
import type { FileRagAdminGet, FileRagAdminSet } from '../state'

export default (set: FileRagAdminSet, _get: FileRagAdminGet) =>
  async () => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const row = await ApiClient.FileRagAdmin.get()
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

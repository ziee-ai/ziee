import { ApiClient } from '@/api-client'
import type { LitSearchAdminGet, LitSearchAdminSet } from '../state'

export default (set: LitSearchAdminSet, _get: LitSearchAdminGet) =>
  async () => {
    try {
      const res = await ApiClient.LitSearch.getConnectors()
      set(s => {
        s.connectors = res.connectors
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load connectors'
      })
    }
  }

import { ApiClient } from '@/api-client'
import type { UpdateConnectorRequest } from '@/api-client/types'
import type { LitSearchAdminGet, LitSearchAdminSet } from '../state'

export default (set: LitSearchAdminSet, _get: LitSearchAdminGet) =>
  async (
    connector: string,
    body: UpdateConnectorRequest,
  ): Promise<void> => {
    set(s => {
      s.savingConnector = connector
      s.error = null
    })
    try {
      const res = await ApiClient.LitSearch.updateConnector({ connector, ...body })
      set(s => {
        s.connectors = res.connectors
        s.savingConnector = null
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.savingConnector = null
      })
      throw error
    }
  }

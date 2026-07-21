import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { CoreMemoryBlocksGet, CoreMemoryBlocksSet } from '../state'

export default (set: CoreMemoryBlocksSet, _get: CoreMemoryBlocksGet) =>
  async (assistantId: string) => {
    // `sync:reconnect` fires for every store regardless of audience; skip the
    // refetch for users without `memory::core::read` (the endpoint would 403).
    if (!hasPermissionNow(Permissions.CoreMemoryRead)) return
    set(s => {
      s.loadingByAssistant[assistantId] = true
      s.error = null
    })
    try {
      const rows = await ApiClient.CoreMemory.list({ assistant_id: assistantId })
      set(s => {
        s.blocksByAssistant[assistantId] = rows
        s.loadingByAssistant[assistantId] = false
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load core memory blocks'
        s.loadingByAssistant[assistantId] = false
      })
    }
  }

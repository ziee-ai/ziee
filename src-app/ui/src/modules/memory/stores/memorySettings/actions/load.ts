import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { emitMemorySettingsUpdated } from '@/modules/memory/events'
import type { MemorySettingsGet, MemorySettingsSet } from '../state'

export default (set: MemorySettingsSet, _get: MemorySettingsGet) =>
  async () => {
    // `sync:reconnect` fires for every store regardless of audience; skip the
    // refetch for users without `memory::read` (the endpoint would 403).
    if (!hasPermissionNow(Permissions.MemoryRead)) return
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const row = await ApiClient.MemorySettings.get()
      set(s => {
        s.settings = row
        s.loading = false
      })
      try {
        await emitMemorySettingsUpdated(row)
      } catch (eventError) {
        console.error('Failed to emit memory settings updated event:', eventError)
      }
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Failed to load settings'
        s.loading = false
      })
    }
  }

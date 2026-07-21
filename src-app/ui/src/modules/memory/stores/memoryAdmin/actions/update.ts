import { ApiClient } from '@/api-client'
import { emitMemoryAdminSettingsUpdated } from '@/modules/memory/events'
import type { MemoryAdminGet, MemoryAdminSet, MemoryAdminUpdatePatch } from '../state'

export default (set: MemoryAdminSet, _get: MemoryAdminGet) =>
  async (patch: MemoryAdminUpdatePatch): Promise<import('@/api-client/types').MemoryAdminSettings> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      // Cast: codegen loses the `null` arm; JSON.stringify writes null vs
      // absent correctly and the backend's deserialize_nullable_field honors both.
      const row = await ApiClient.MemoryAdmin.update(
        patch as Parameters<typeof ApiClient.MemoryAdmin.update>[0],
      )
      set(s => {
        s.settings = row
        s.saving = false
      })
      try {
        await emitMemoryAdminSettingsUpdated(row)
      } catch (eventError) {
        console.error('Failed to emit memory admin settings updated event:', eventError)
      }
      return row
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.saving = false
      })
      throw error
    }
  }

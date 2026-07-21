import { ApiClient } from '@/api-client'
import type { UpdateMemoryRequest, UserMemory } from '@/api-client/types'
import type { MemoriesGet, MemoriesSet } from '../state'
import emitMemoryUpdatedFactory from './emitMemoryUpdated'

export default (set: MemoriesSet, get: MemoriesGet) => {
  const emitMemoryUpdated = emitMemoryUpdatedFactory(set, get)
  return async (
    id: string,
    patch: Omit<UpdateMemoryRequest, never>,
  ): Promise<UserMemory> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const row = await ApiClient.Memory.update({ id, ...patch })
      set(s => {
        const idx = s.memories.findIndex(m => m.id === id)
        if (idx >= 0) s.memories[idx] = row
        s.saving = false
      })
      await emitMemoryUpdated(row)
      return row
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Update failed'
        s.saving = false
      })
      throw error
    }
  }
}

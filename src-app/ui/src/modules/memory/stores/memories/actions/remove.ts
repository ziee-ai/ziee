import { ApiClient } from '@/api-client'
import type { MemoriesGet, MemoriesSet } from '../state'
import emitMemoryDeletedFactory from './emitMemoryDeleted'

export default (set: MemoriesSet, get: MemoriesGet) => {
  const emitMemoryDeleted = emitMemoryDeletedFactory(set, get)
  return async (id: string): Promise<void> => {
    try {
      await ApiClient.Memory.delete({ id })
      set(s => {
        s.memories = s.memories.filter(m => m.id !== id)
        s.total = Math.max(0, s.total - 1)
      })
      await emitMemoryDeleted(id)
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Delete failed'
      })
      throw error
    }
  }
}

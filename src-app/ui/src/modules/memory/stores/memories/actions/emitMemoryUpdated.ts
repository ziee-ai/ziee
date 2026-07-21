import { emitMemoryUpdated } from '@/modules/memory/events'
import type { MemoriesSet, MemoriesGet } from '../state'
import type { UserMemory } from '@/api-client/types'

export default (_set: MemoriesSet, _get: MemoriesGet) =>
  async (row: UserMemory) => {
    try {
      await emitMemoryUpdated(row)
    } catch (eventError) {
      console.error('Failed to emit memory updated event:', eventError)
    }
  }

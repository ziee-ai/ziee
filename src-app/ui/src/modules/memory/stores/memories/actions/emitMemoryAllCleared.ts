import { emitMemoryAllCleared } from '@/modules/memory/events'
import type { MemoriesSet, MemoriesGet } from '../state'

export default (_set: MemoriesSet, _get: MemoriesGet) =>
  async (count: number) => {
    try {
      await emitMemoryAllCleared(count)
    } catch (eventError) {
      console.error('Failed to emit memory all-cleared event:', eventError)
    }
  }

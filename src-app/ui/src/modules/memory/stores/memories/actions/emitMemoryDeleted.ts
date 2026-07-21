import { emitMemoryDeleted } from '@/modules/memory/events'
import type { MemoriesSet, MemoriesGet } from '../state'

export default (_set: MemoriesSet, _get: MemoriesGet) =>
  async (id: string) => {
    try {
      await emitMemoryDeleted(id)
    } catch (eventError) {
      console.error('Failed to emit memory deleted event:', eventError)
    }
  }

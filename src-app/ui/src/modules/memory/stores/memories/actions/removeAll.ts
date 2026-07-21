import { ApiClient } from '@/api-client'
import type { DeleteAllResponse } from '@/api-client/types'
import type { MemoriesGet, MemoriesSet } from '../state'
import emitMemoryAllClearedFactory from './emitMemoryAllCleared'

export default (set: MemoriesSet, get: MemoriesGet) => {
  const emitMemoryAllCleared = emitMemoryAllClearedFactory(set, get)
  return async (): Promise<number> => {
    try {
      const body: DeleteAllResponse = await ApiClient.Memory.deleteAll()
      set(s => {
        s.memories = []
        s.total = 0
        s.currentPage = 1
      })
      await emitMemoryAllCleared(body.deleted)
      return body.deleted
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Delete-all failed'
      })
      throw error
    }
  }
}

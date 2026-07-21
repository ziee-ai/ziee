import { ApiClient } from '@/api-client'
import type { CreateMemoryRequest, UserMemory } from '@/api-client/types'
import type { MemoriesGet, MemoriesSet } from '../state'
import emitMemoryCreatedFactory from './emitMemoryCreated'

export default (set: MemoriesSet, get: MemoriesGet) => {
  const emitMemoryCreated = emitMemoryCreatedFactory(set, get)
  return async (
    content: string,
    importance?: number,
    kind?: string,
  ): Promise<UserMemory> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const req: CreateMemoryRequest = {
        content,
        importance: importance ?? 50,
        kind: kind ?? 'fact',
        metadata: {},
      }
      const row = await ApiClient.Memory.create(req)
      set(s => {
        s.memories.unshift(row)
        s.total += 1
        s.saving = false
      })
      await emitMemoryCreated(row)
      return row
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Create failed'
        s.saving = false
      })
      throw error
    }
  }
}

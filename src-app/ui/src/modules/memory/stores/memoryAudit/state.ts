import type { StoreSet } from '@ziee/framework/store-kit'
import type { MemoryAuditEntry } from '@/api-client/types'

export const memoryAuditState = {
  entries: [] as MemoryAuditEntry[],
  loading: false,
  limit: 100,
  error: null as string | null,
}

export type MemoryAuditState = typeof memoryAuditState
export type MemoryAuditSet = StoreSet<MemoryAuditState>
export type MemoryAuditGet = () => MemoryAuditState

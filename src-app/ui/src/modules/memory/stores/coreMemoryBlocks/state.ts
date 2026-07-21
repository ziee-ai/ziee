import type { StoreSet } from '@ziee/framework/store-kit'
import type { CoreMemoryBlock } from '@/api-client/types'

export const coreMemoryBlocksState = {
  // Keyed by assistant_id so multiple editors don't clobber each other.
  blocksByAssistant: {} as Record<string, CoreMemoryBlock[]>,
  loadingByAssistant: {} as Record<string, boolean>,
  error: null as string | null,
}

export type CoreMemoryBlocksState = typeof coreMemoryBlocksState
export type CoreMemoryBlocksSet = StoreSet<CoreMemoryBlocksState>
export type CoreMemoryBlocksGet = () => CoreMemoryBlocksState

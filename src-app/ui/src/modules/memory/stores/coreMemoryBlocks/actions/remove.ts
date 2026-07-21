import { ApiClient } from '@/api-client'
import type { CoreMemoryBlocksGet, CoreMemoryBlocksSet } from '../state'

export default (set: CoreMemoryBlocksSet, _get: CoreMemoryBlocksGet) =>
  async (assistantId: string, blockLabel: string) => {
    try {
      await ApiClient.CoreMemory.delete({ assistant_id: assistantId, block_label: blockLabel })
      set(s => {
        const current = s.blocksByAssistant[assistantId] ?? []
        s.blocksByAssistant[assistantId] = current.filter(b => b.block_label !== blockLabel)
      })
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Delete failed'
      })
      throw error
    }
  }

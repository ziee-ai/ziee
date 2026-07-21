import { ApiClient } from '@/api-client'
import type { CoreMemoryBlock } from '@/api-client/types'
import type { CoreMemoryBlocksGet, CoreMemoryBlocksSet } from '../state'

export default (set: CoreMemoryBlocksSet, _get: CoreMemoryBlocksGet) =>
  async (input: {
    assistant_id: string
    block_label: string
    content: string
    char_limit: number
  }): Promise<CoreMemoryBlock> => {
    set(s => {
      s.loadingByAssistant[input.assistant_id] = true
      s.error = null
    })
    try {
      const block = await ApiClient.CoreMemory.upsert(input)
      set(s => {
        const current = s.blocksByAssistant[input.assistant_id] ?? []
        const idx = current.findIndex(b => b.block_label === input.block_label)
        if (idx >= 0) current[idx] = block
        else current.push(block)
        s.blocksByAssistant[input.assistant_id] = current
        s.loadingByAssistant[input.assistant_id] = false
      })
      return block
    } catch (error) {
      set(s => {
        s.error = error instanceof Error ? error.message : 'Save failed'
        s.loadingByAssistant[input.assistant_id] = false
      })
      throw error
    }
  }

import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { CoreMemoryBlock } from '@/api-client/types'

interface CoreMemoryBlocksStore {
  // Keyed by assistant_id so multiple editors (or tab-switching) don't
  // clobber each other. Most flows only ever populate one entry.
  blocksByAssistant: Record<string, CoreMemoryBlock[]>
  loadingByAssistant: Record<string, boolean>
  error: string | null

  load: (assistantId: string) => Promise<void>
  upsert: (input: {
    assistant_id: string
    block_label: string
    content: string
    char_limit: number
  }) => Promise<CoreMemoryBlock>
  remove: (assistantId: string, blockLabel: string) => Promise<void>
}

export const useCoreMemoryBlocksStore = create<CoreMemoryBlocksStore>()(
  subscribeWithSelector(
    immer((set, _get) => ({
      blocksByAssistant: {},
      loadingByAssistant: {},
      error: null,

      load: async (assistantId: string): Promise<void> => {
        set(s => {
          s.loadingByAssistant[assistantId] = true
          s.error = null
        })
        try {
          const rows = await ApiClient.CoreMemory.list({
            assistant_id: assistantId,
          })
          set(s => {
            s.blocksByAssistant[assistantId] = rows
            s.loadingByAssistant[assistantId] = false
          })
        } catch (error) {
          set(s => {
            s.error =
              error instanceof Error
                ? error.message
                : 'Failed to load core memory blocks'
            s.loadingByAssistant[assistantId] = false
          })
        }
      },

      upsert: async (input): Promise<CoreMemoryBlock> => {
        try {
          const block = await ApiClient.CoreMemory.upsert(input)
          set(s => {
            const current = s.blocksByAssistant[input.assistant_id] ?? []
            const idx = current.findIndex(
              b => b.block_label === input.block_label,
            )
            if (idx >= 0) {
              current[idx] = block
            } else {
              current.push(block)
            }
            s.blocksByAssistant[input.assistant_id] = current
          })
          return block
        } catch (error) {
          set(s => {
            s.error =
              error instanceof Error ? error.message : 'Save failed'
          })
          throw error
        }
      },

      remove: async (assistantId, blockLabel): Promise<void> => {
        try {
          await ApiClient.CoreMemory.delete({
            assistant_id: assistantId,
            block_label: blockLabel,
          })
          set(s => {
            const current = s.blocksByAssistant[assistantId] ?? []
            s.blocksByAssistant[assistantId] = current.filter(
              b => b.block_label !== blockLabel,
            )
          })
        } catch (error) {
          set(s => {
            s.error =
              error instanceof Error ? error.message : 'Delete failed'
          })
          throw error
        }
      },
    })),
  ),
)

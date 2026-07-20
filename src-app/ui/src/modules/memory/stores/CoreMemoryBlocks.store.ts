import { ApiClient } from '@/api-client'
import { type CoreMemoryBlock } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

export const CoreMemoryBlocks = defineStore('CoreMemoryBlocks', {
  immer: true,
  state: {
    // Keyed by assistant_id so multiple editors don't clobber each other.
    blocksByAssistant: {} as Record<string, CoreMemoryBlock[]>,
    loadingByAssistant: {} as Record<string, boolean>,
    error: null as string | null,
  },
  actions: set => ({
    load: async (assistantId: string) => {
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users without `memory::core::read` (the endpoint would 403).
      if (!hasPermissionNow(Permissions.CoreMemoryRead)) return
      set(s => {
        s.loadingByAssistant[assistantId] = true
        s.error = null
      })
      try {
        const rows = await ApiClient.CoreMemory.list({ assistant_id: assistantId })
        set(s => {
          s.blocksByAssistant[assistantId] = rows
          s.loadingByAssistant[assistantId] = false
        })
      } catch (error) {
        set(s => {
          s.error = error instanceof Error ? error.message : 'Failed to load core memory blocks'
          s.loadingByAssistant[assistantId] = false
        })
      }
    },
    upsert: async (input: {
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
    },
    remove: async (assistantId: string, blockLabel: string) => {
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
    },
  }),
  init: ({ on, get, actions }) => {
    // Refresh open editors on reconnect / core-memory sync. load() is
    // permission-gated internally (memory::core::read).
    const reloadAll = () => {
      Object.keys(get().blocksByAssistant).forEach(id => void actions.load(id))
    }
    on('sync:assistant_core_memory', reloadAll)
    on('sync:reconnect', reloadAll)
  },
})

export const useCoreMemoryBlocksStore = CoreMemoryBlocks.store

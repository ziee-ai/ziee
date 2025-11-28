import { enableMapSet } from 'immer'
import { createExtensionStore } from '../../core/extensions'

// Enable Map support in Immer
enableMapSet()

/**
 * MCP tool call state
 */
export interface McpToolCall {
  tool_use_id: string
  server: string
  tool_name: string
  status: 'started' | 'pending_approval' | 'completed' | 'error'
  input?: unknown
  result?: unknown
  error?: string
}

/**
 * MCP extension store
 * Combines state and actions
 */
interface McpStore {
  // State
  /** Map of tool calls by tool_use_id */
  toolCalls: Map<string, McpToolCall>

  // Actions
  /** Add a new tool call */
  addToolCall: (toolCall: McpToolCall) => void
  /** Update an existing tool call */
  updateToolCall: (toolUseId: string, updates: Partial<McpToolCall>) => void
  /** Get a tool call by ID */
  getToolCall: (toolUseId: string) => McpToolCall | undefined
  /** Get all active tool calls (started or pending approval) */
  getActiveCalls: () => McpToolCall[]
  /** Clear all tool calls for current conversation */
  clearToolCalls: () => void
}

/**
 * Create MCP extension store
 * Independent Zustand store with full reactivity
 * Accessible via Stores.Chat.McpStore
 */
export const createMcpStore = () =>
  createExtensionStore<McpStore>((set, get) => ({
    // State
    toolCalls: new Map<string, McpToolCall>(),

    // Actions
    /**
     * Add a new tool call
     */
    addToolCall: (toolCall: McpToolCall) => {
      set(state => {
        state.toolCalls.set(toolCall.tool_use_id, toolCall)
      })
    },

    /**
     * Update an existing tool call
     */
    updateToolCall: (toolUseId: string, updates: Partial<McpToolCall>) => {
      set(state => {
        const existing = state.toolCalls.get(toolUseId)
        if (existing) {
          state.toolCalls.set(toolUseId, {
            ...existing,
            ...updates,
          })
        }
      })
    },

    /**
     * Get a tool call by ID
     */
    getToolCall: (toolUseId: string): McpToolCall | undefined => {
      const state = get()
      return state.toolCalls.get(toolUseId)
    },

    /**
     * Get all active tool calls (started or pending approval)
     */
    getActiveCalls: (): McpToolCall[] => {
      const state = get()
      const allCalls = Array.from(state.toolCalls.values())
      return allCalls.filter(
        call => call.status === 'started' || call.status === 'pending_approval',
      )
    },

    /**
     * Clear all tool calls for current conversation
     */
    clearToolCalls: () => {
      set(state => {
        state.toolCalls.clear()
      })
    },
  }))

/**
 * Augment ChatExtensionStores with McpStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    McpStore: ReturnType<typeof createMcpStore>
  }
}

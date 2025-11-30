import { enableMapSet } from 'immer'
import { createExtensionStore } from '../../core/extensions'
import type { ToolApprovalDecision, McpServerConfig } from '@/api-client/types'

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
 * Server selection state
 * Maps server_id to selected tools (empty array = all tools)
 */
interface ServerSelection {
  server_id: string
  tools: string[] // Empty = all tools from server
}

/**
 * MCP extension store
 * Combines state and actions
 */
interface McpStore {
  // State
  /** Map of tool calls by tool_use_id */
  toolCalls: Map<string, McpToolCall>
  /** Pending approval decisions (to be sent with next message) */
  approvalDecisions: ToolApprovalDecision[]
  /** Selected servers and their tools */
  selectedServers: Map<string, ServerSelection>

  // Tool call actions
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

  // Approval decision actions
  /** Add an approval decision (will be sent with next message) */
  addApprovalDecision: (decision: ToolApprovalDecision) => void
  /** Get all pending approval decisions */
  getApprovalDecisions: () => ToolApprovalDecision[]
  /** Clear all approval decisions (after sending) */
  clearApprovalDecisions: () => void

  // Server selection actions
  /** Select a server (tools=[] means all tools) */
  selectServer: (serverId: string, tools?: string[]) => void
  /** Deselect a server */
  deselectServer: (serverId: string) => void
  /** Toggle a specific tool for a server */
  toggleServerTool: (serverId: string, toolName: string) => void
  /** Get selected servers config for request */
  getSelectedServersConfig: () => McpServerConfig[]
  /** Clear all server selections */
  clearSelection: () => void
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
    approvalDecisions: [],
    selectedServers: new Map<string, ServerSelection>(),

    // Tool call actions
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

    // Approval decision actions
    /**
     * Add an approval decision (will be sent with next message)
     */
    addApprovalDecision: (decision: ToolApprovalDecision) => {
      set(state => {
        state.approvalDecisions.push(decision)
      })
      console.log(
        '[MCP Store] Added approval decision:',
        decision.decision,
        decision.tool_use_id,
      )
    },

    /**
     * Get all pending approval decisions
     */
    getApprovalDecisions: (): ToolApprovalDecision[] => {
      return get().approvalDecisions
    },

    /**
     * Clear all approval decisions (after sending)
     */
    clearApprovalDecisions: () => {
      set(state => {
        state.approvalDecisions = []
      })
      console.log('[MCP Store] Cleared approval decisions')
    },

    // Server selection actions
    /**
     * Select a server (tools=[] means all tools)
     */
    selectServer: (serverId: string, tools: string[] = []) => {
      set(state => {
        state.selectedServers.set(serverId, {
          server_id: serverId,
          tools,
        })
      })
      console.log('[MCP Store] Selected server:', serverId, 'tools:', tools)
    },

    /**
     * Deselect a server
     */
    deselectServer: (serverId: string) => {
      set(state => {
        state.selectedServers.delete(serverId)
      })
      console.log('[MCP Store] Deselected server:', serverId)
    },

    /**
     * Toggle a specific tool for a server
     */
    toggleServerTool: (serverId: string, toolName: string) => {
      set(state => {
        const selection = state.selectedServers.get(serverId)
        if (!selection) return

        const toolIndex = selection.tools.indexOf(toolName)
        let newTools: string[]

        if (toolIndex >= 0) {
          // Tool is selected, remove it
          newTools = selection.tools.filter((_, index) => index !== toolIndex)
        } else {
          // Tool not selected, add it
          newTools = [...selection.tools, toolName]
        }

        state.selectedServers.set(serverId, {
          server_id: serverId,
          tools: newTools,
        })
      })
    },

    /**
     * Get selected servers config for request
     */
    getSelectedServersConfig: (): McpServerConfig[] => {
      const selections = Array.from(get().selectedServers.values())
      return selections.map(sel => ({
        server_id: sel.server_id,
        tools: sel.tools.length > 0 ? sel.tools : undefined,
      }))
    },

    /**
     * Clear all server selections
     */
    clearSelection: () => {
      set(state => {
        state.selectedServers.clear()
      })
      console.log('[MCP Store] Cleared all server selections')
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

import { enableMapSet } from 'immer'
import { createExtensionStore } from '@/modules/chat/core/extensions'
import type { ToolApprovalDecision, McpServerConfig, AutoApprovedServer, DisabledServer, UserMcpDefaultsResponse, LoopSettings, ToolIdentifier, PerToolLimit, SSEChatStreamMcpElicitationRequiredData } from '@/api-client/types'
import { ApiClient } from '@/api-client'

// Enable Map support in Immer
enableMapSet()

/**
 * Elicitation request state — a pending form the user needs to fill in
 */
export interface ElicitationRequestState extends SSEChatStreamMcpElicitationRequiredData {
  status: 'pending' | 'accepted' | 'declined' | 'cancelled'
  /** Submitted field values — only set when status = 'accepted' */
  response_content?: Record<string, unknown>
}

/**
 * MCP tool call state
 */
export interface McpToolCall {
  tool_use_id: string
  /** Server display name */
  server: string
  /** Server UUID */
  server_id?: string
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
 * Per-conversation MCP configuration
 * Persisted and loaded from backend
 */
interface ConversationMcpConfig {
  /** Selected servers with tool filtering */
  selectedServers: Map<string, ServerSelection>
  /** Disabled servers (persisted to backend) - allows all servers by default */
  disabledServers?: DisabledServer[]
  /** Approval mode from conversation_mcp_settings */
  approvalMode?: 'disabled' | 'auto_approve' | 'manual_approve'
  /** Auto-approved tools grouped by server */
  autoApprovedTools?: AutoApprovedServer[]
  /** Loop settings for controlling iteration behavior */
  loopSettings?: LoopSettings
}

/** Special key for pending (new conversation) config */
export const PENDING_CONVERSATION_KEY = '__pending__'

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
  /** Per-conversation MCP configuration (persisted) */
  conversationConfigs: Map<string, ConversationMcpConfig>
  /** Current conversation ID (null for new conversations) */
  currentConversationId: string | null
  /** Selected servers and their tools (computed from current conversation config) */
  selectedServers: Map<string, ServerSelection>
  /** User's default MCP settings (loaded on init) */
  userDefaults: UserMcpDefaultsResponse | null
  /** Whether user defaults have been loaded */
  userDefaultsLoaded: boolean
  /** Whether the config modal is visible */
  configModalVisible: boolean
  /** Pending elicitation requests keyed by message_id */
  elicitationRequests: Map<string, ElicitationRequestState>

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

  // Conversation config actions
  /** Set current conversation ID and load its config */
  setCurrentConversation: (conversationId: string | null) => void
  /** Load conversation config (from backend or create default) */
  loadConversationConfig: (conversationId: string, config?: ConversationMcpConfig) => void
  /** Save conversation config changes (availableServerIds used to compute disabled_servers, serverToolsMap used to persist partial tool selections) */
  saveConversationConfig: (conversationId: string, availableServerIds?: string[], serverToolsMap?: Map<string, string[]>, updateAutoApproved?: boolean) => Promise<void>
  /** Get or create pending config for new conversations */
  getOrCreatePendingConfig: () => ConversationMcpConfig
  /** Transfer pending config to a real conversation ID */
  transferPendingConfig: (conversationId: string) => void
  /** Set approval mode for a conversation (or pending) */
  setApprovalMode: (conversationId: string | null, mode: 'disabled' | 'auto_approve' | 'manual_approve') => void
  /** Toggle auto-approved status for a tool (conversationId can be null for pending) */
  toggleAutoApprovedTool: (conversationId: string | null, serverId: string, toolName: string) => void
  /** Check if a tool is auto-approved */
  isToolAutoApproved: (serverId: string, toolName: string) => boolean

  // Loop settings actions
  /** Set loop settings (partial update) */
  setLoopSettings: (conversationId: string | null, settings: Partial<LoopSettings>) => void
  /** Add a tool to stop_when_tools_called */
  addStopWhenToolCalled: (conversationId: string | null, tool: ToolIdentifier) => void
  /** Remove a tool from stop_when_tools_called */
  removeStopWhenToolCalled: (conversationId: string | null, serverId: string, toolName: string) => void
  /** Add a per-tool iteration limit */
  addPerToolLimit: (conversationId: string | null, limit: PerToolLimit) => void
  /** Remove a per-tool iteration limit */
  removePerToolLimit: (conversationId: string | null, serverId: string, toolName: string) => void
  /** Update a per-tool iteration limit */
  updatePerToolLimit: (conversationId: string | null, serverId: string, toolName: string, maxIteration: number) => void

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
  /** Set enabled servers from a list of IDs (deselects all others) */
  setEnabledServers: (serverIds: string[]) => void

  // Initialization methods
  __init__: {
    userDefaults: () => Promise<void>
  }

  // User defaults actions
  /** Load user defaults from backend */
  loadUserDefaults: () => Promise<void>
  /** Save current config as user defaults (availableServerIds used to compute disabled_servers) */
  saveUserDefaults: (conversationId: string | null, availableServerIds: string[], updateAutoApproved?: boolean) => Promise<void>
  /** Apply user defaults to pending config (for new conversations) */
  applyUserDefaultsToPending: (availableServerIds: string[]) => void

  // Config modal actions
  /** Open the config modal */
  openConfigModal: () => void
  /** Close the config modal */
  closeConfigModal: () => void

  // Elicitation actions
  /** Add a new elicitation request (called when mcpElicitationRequired SSE event arrives) */
  addElicitationRequest: (request: SSEChatStreamMcpElicitationRequiredData) => void
  /** Respond to an elicitation (POST to backend, then remove from map) */
  resolveElicitation: (elicitation_id: string, action: 'accept' | 'decline' | 'cancel', content?: Record<string, unknown>) => Promise<void>
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
    conversationConfigs: new Map<string, ConversationMcpConfig>(),
    currentConversationId: null,
    selectedServers: new Map<string, ServerSelection>(),
    userDefaults: null,
    userDefaultsLoaded: false,
    configModalVisible: false,
    elicitationRequests: new Map<string, ElicitationRequestState>(),

    // Initialization methods
    __init__: {
      userDefaults: () => get().loadUserDefaults(),
    },

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

    // Conversation config actions
    /**
     * Set current conversation ID and load its config
     */
    setCurrentConversation: (conversationId: string | null) => {
      set(state => {
        state.currentConversationId = conversationId

        // Determine which config key to use
        const configKey = conversationId || PENDING_CONVERSATION_KEY

        // Load selected servers from conversation config (or pending)
        if (state.conversationConfigs.has(configKey)) {
          const config = state.conversationConfigs.get(configKey)!
          state.selectedServers = new Map(config.selectedServers)
        } else if (!conversationId) {
          // New conversation without pending config - create one with user defaults if available
          const defaults = state.userDefaults
          const pendingConfig: ConversationMcpConfig = {
            selectedServers: new Map(),
            disabledServers: defaults?.disabled_servers || [],
            approvalMode: (defaults?.approval_mode as 'disabled' | 'auto_approve' | 'manual_approve') || 'manual_approve',
            autoApprovedTools: defaults?.auto_approved_tools || [],
            loopSettings: defaults?.loop_settings,
          }
          state.conversationConfigs.set(PENDING_CONVERSATION_KEY, pendingConfig)
          state.selectedServers = new Map()
        } else {
          // No config yet, reset to empty
          state.selectedServers = new Map()
        }
      })
      console.log('[MCP Store] Set current conversation:', conversationId)
    },

    /**
     * Load conversation config (from backend or create default)
     */
    loadConversationConfig: (conversationId: string, config?: ConversationMcpConfig) => {
      set(state => {
        if (config) {
          state.conversationConfigs.set(conversationId, config)
        } else {
          // Create default config
          state.conversationConfigs.set(conversationId, {
            selectedServers: new Map(),
            approvalMode: 'manual_approve',
            autoApprovedTools: [],
          })
        }

        // If this is current conversation, update selectedServers
        if (state.currentConversationId === conversationId) {
          const loadedConfig = state.conversationConfigs.get(conversationId)!
          state.selectedServers = new Map(loadedConfig.selectedServers)
        }
      })
      console.log('[MCP Store] Loaded conversation config:', conversationId)
    },

    /**
     * Save conversation config changes
     * Saves approval settings and disabled servers to backend
     */
    saveConversationConfig: async (conversationId: string, availableServerIds?: string[], serverToolsMap?: Map<string, string[]>, updateAutoApproved?: boolean) => {
      const state = get()
      const config = state.conversationConfigs.get(conversationId)

      if (!config) {
        console.warn('[MCP Store] No config to save for:', conversationId)
        return
      }

      // Compute disabled_servers from selectedServers (inverted logic)
      // Any server NOT in selectedServers should be in disabled_servers
      let disabledServers: DisabledServer[] = []
      if (availableServerIds && availableServerIds.length > 0) {
        const selectedServerIds = new Set(config.selectedServers.keys())
        disabledServers = availableServerIds
          .filter(id => !selectedServerIds.has(id))
          .map(id => ({ server_id: id, tools: [] })) // Empty tools = entire server disabled
      }

      // For partially selected servers (specific tools chosen), compute disabled tools
      // and add them to disabled_servers with the specific tool names
      if (serverToolsMap) {
        for (const [serverId, selection] of config.selectedServers.entries()) {
          if (selection.tools.length > 0) {
            const allTools = serverToolsMap.get(serverId) || []
            const disabledTools = allTools.filter(t => !selection.tools.includes(t))
            if (disabledTools.length > 0) {
              disabledServers.push({ server_id: serverId, tools: disabledTools })
            }
          }
        }
      }

      // Also include any previously saved disabled servers that aren't in available list
      // (to preserve settings for servers that might be temporarily unavailable)
      const existingDisabled = config.disabledServers || []
      const availableSet = new Set(availableServerIds || [])
      const unavailableDisabled = existingDisabled.filter(d => !availableSet.has(d.server_id))
      disabledServers = [...disabledServers, ...unavailableDisabled]

      // Call backend API to persist settings
      const { ApiClient } = await import('@/api-client')
      await ApiClient.Conversation.updateMcpSettings({
        id: conversationId,
        approval_mode: config.approvalMode || 'manual_approve',
        // Only send auto_approved_tools when explicitly changing approvals — backend COALESCE preserves DB value otherwise
        ...(updateAutoApproved ? { auto_approved_tools: config.autoApprovedTools } : {}),
        disabled_servers: disabledServers,
        loop_settings: config.loopSettings,
      })

      // Update local state with the computed disabled servers
      set(state => {
        const existingConfig = state.conversationConfigs.get(conversationId)
        if (existingConfig) {
          state.conversationConfigs.set(conversationId, {
            ...existingConfig,
            disabledServers,
          })
        }
      })

      console.log('[MCP Store] Saved conversation config:', conversationId, {
        approvalMode: config.approvalMode,
        autoApprovedTools: config.autoApprovedTools?.length || 0,
        disabledServers: disabledServers.length,
      })
    },

    /**
     * Get or create pending config for new conversations
     */
    getOrCreatePendingConfig: (): ConversationMcpConfig => {
      const state = get()
      let config = state.conversationConfigs.get(PENDING_CONVERSATION_KEY)
      if (!config) {
        config = {
          selectedServers: new Map(),
          disabledServers: [],
          approvalMode: 'manual_approve',
          autoApprovedTools: [],
        }
        set(s => {
          s.conversationConfigs.set(PENDING_CONVERSATION_KEY, config!)
        })
      }
      return config
    },

    /**
     * Transfer pending config to a real conversation ID
     */
    transferPendingConfig: (conversationId: string) => {
      set(state => {
        const pendingConfig = state.conversationConfigs.get(PENDING_CONVERSATION_KEY)
        if (pendingConfig) {
          // Copy pending config to new conversation
          state.conversationConfigs.set(conversationId, {
            ...pendingConfig,
            selectedServers: new Map(pendingConfig.selectedServers),
          })
          // Clear pending config
          state.conversationConfigs.delete(PENDING_CONVERSATION_KEY)
          console.log('[MCP Store] Transferred pending config to conversation:', conversationId)
        }
      })
    },

    /**
     * Set approval mode for a conversation (or pending if conversationId is null)
     */
    setApprovalMode: (conversationId: string | null, mode: 'disabled' | 'auto_approve' | 'manual_approve') => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        let config = state.conversationConfigs.get(configKey)

        // Create pending config if it doesn't exist (for new conversations)
        if (!config && !conversationId) {
          config = {
            selectedServers: new Map(),
            disabledServers: [],
            approvalMode: 'manual_approve',
            autoApprovedTools: [],
          }
          state.conversationConfigs.set(PENDING_CONVERSATION_KEY, config)
        }

        if (config) {
          config.approvalMode = mode
        }
      })
    },

    /**
     * Toggle auto-approved status for a tool
     * Uses structured format: [{server_id, tools: []}]
     * conversationId can be null for pending (new conversation)
     */
    toggleAutoApprovedTool: (conversationId: string | null, serverId: string, toolName: string) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        let config = state.conversationConfigs.get(configKey)

        // Create pending config if it doesn't exist (for new conversations)
        if (!config && !conversationId) {
          config = {
            selectedServers: new Map(),
            disabledServers: [],
            approvalMode: 'manual_approve',
            autoApprovedTools: [],
          }
          state.conversationConfigs.set(PENDING_CONVERSATION_KEY, config)
        }

        if (!config) return

        const autoApproved = config.autoApprovedTools || []

        // Find existing server entry
        const serverIndex = autoApproved.findIndex(s => s.server_id === serverId)

        if (serverIndex >= 0) {
          // Server exists, toggle tool in its tools array
          const server = autoApproved[serverIndex]
          const toolIndex = server.tools.indexOf(toolName)

          if (toolIndex >= 0) {
            // Tool exists, remove it
            const newTools = server.tools.filter((_, i) => i !== toolIndex)
            if (newTools.length === 0) {
              // No more tools for this server, remove server entry
              config.autoApprovedTools = autoApproved.filter((_, i) => i !== serverIndex)
            } else {
              // Update server with remaining tools
              config.autoApprovedTools = autoApproved.map((s, i) =>
                i === serverIndex ? { ...s, tools: newTools } : s,
              )
            }
          } else {
            // Tool doesn't exist, add it
            config.autoApprovedTools = autoApproved.map((s, i) =>
              i === serverIndex ? { ...s, tools: [...s.tools, toolName] } : s,
            )
          }
        } else {
          // Server doesn't exist, create new entry
          config.autoApprovedTools = [...autoApproved, { server_id: serverId, tools: [toolName] }]
        }
      })
    },

    /**
     * Check if a tool is auto-approved for current conversation (or pending)
     */
    isToolAutoApproved: (serverId: string, toolName: string) => {
      const state = get()
      const configKey = state.currentConversationId || PENDING_CONVERSATION_KEY

      const config = state.conversationConfigs.get(configKey)
      if (!config || !config.autoApprovedTools) return false

      // Find server entry and check if tool is in its tools array
      const serverEntry = config.autoApprovedTools.find(s => s.server_id === serverId)
      return serverEntry ? serverEntry.tools.includes(toolName) : false
    },

    // Loop settings actions
    /**
     * Set loop settings (partial update)
     */
    setLoopSettings: (conversationId: string | null, settings: Partial<LoopSettings>) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        let config = state.conversationConfigs.get(configKey)

        // Create config if it doesn't exist (for both new and existing conversations)
        if (!config) {
          config = {
            selectedServers: new Map(),
            disabledServers: [],
            approvalMode: 'manual_approve',
            autoApprovedTools: [],
            loopSettings: {},
          }
          state.conversationConfigs.set(configKey, config)
        }

        config.loopSettings = { ...config.loopSettings, ...settings }
      })
    },

    /**
     * Add a tool to stop_when_tools_called
     */
    addStopWhenToolCalled: (conversationId: string | null, tool: ToolIdentifier) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        let config = state.conversationConfigs.get(configKey)

        // Create config if it doesn't exist (for both new and existing conversations)
        if (!config) {
          config = {
            selectedServers: new Map(),
            disabledServers: [],
            approvalMode: 'manual_approve',
            autoApprovedTools: [],
            loopSettings: {},
          }
          state.conversationConfigs.set(configKey, config)
        }

        const current = config.loopSettings?.stop_when_tools_called || []
        // Avoid duplicates
        if (!current.some(t => t.server_id === tool.server_id && t.tool_name === tool.tool_name)) {
          config.loopSettings = {
            ...config.loopSettings,
            stop_when_tools_called: [...current, tool],
          }
        }
      })
    },

    /**
     * Remove a tool from stop_when_tools_called
     */
    removeStopWhenToolCalled: (conversationId: string | null, serverId: string, toolName: string) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        const config = state.conversationConfigs.get(configKey)

        if (config && config.loopSettings?.stop_when_tools_called) {
          config.loopSettings = {
            ...config.loopSettings,
            stop_when_tools_called: config.loopSettings.stop_when_tools_called.filter(
              t => !(t.server_id === serverId && t.tool_name === toolName)
            ),
          }
        }
      })
    },

    /**
     * Add a per-tool iteration limit
     */
    addPerToolLimit: (conversationId: string | null, limit: PerToolLimit) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        let config = state.conversationConfigs.get(configKey)

        // Create config if it doesn't exist (for both new and existing conversations)
        if (!config) {
          config = {
            selectedServers: new Map(),
            disabledServers: [],
            approvalMode: 'manual_approve',
            autoApprovedTools: [],
            loopSettings: {},
          }
          state.conversationConfigs.set(configKey, config)
        }

        const current = config.loopSettings?.per_tool_max_iteration || []
        // Avoid duplicates - update existing if found
        const existingIndex = current.findIndex(
          t => t.server_id === limit.server_id && t.tool_name === limit.tool_name
        )
        if (existingIndex >= 0) {
          // Update existing
          const updated = [...current]
          updated[existingIndex] = limit
          config.loopSettings = {
            ...config.loopSettings,
            per_tool_max_iteration: updated,
          }
        } else {
          // Add new
          config.loopSettings = {
            ...config.loopSettings,
            per_tool_max_iteration: [...current, limit],
          }
        }
      })
    },

    /**
     * Remove a per-tool iteration limit
     */
    removePerToolLimit: (conversationId: string | null, serverId: string, toolName: string) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        const config = state.conversationConfigs.get(configKey)

        if (config && config.loopSettings?.per_tool_max_iteration) {
          config.loopSettings = {
            ...config.loopSettings,
            per_tool_max_iteration: config.loopSettings.per_tool_max_iteration.filter(
              t => !(t.server_id === serverId && t.tool_name === toolName)
            ),
          }
        }
      })
    },

    /**
     * Update a per-tool iteration limit
     */
    updatePerToolLimit: (conversationId: string | null, serverId: string, toolName: string, maxIteration: number) => {
      set(state => {
        const configKey = conversationId || PENDING_CONVERSATION_KEY
        const config = state.conversationConfigs.get(configKey)

        if (config && config.loopSettings?.per_tool_max_iteration) {
          config.loopSettings = {
            ...config.loopSettings,
            per_tool_max_iteration: config.loopSettings.per_tool_max_iteration.map(t =>
              t.server_id === serverId && t.tool_name === toolName
                ? { ...t, max_iteration: maxIteration }
                : t
            ),
          }
        }
      })
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

        // Update conversation config (or pending config)
        const configKey = state.currentConversationId || PENDING_CONVERSATION_KEY
        const config = state.conversationConfigs.get(configKey)
        if (config) {
          config.selectedServers.set(serverId, { server_id: serverId, tools })
        }
      })
      console.log('[MCP Store] Selected server:', serverId, 'tools:', tools)
    },

    /**
     * Deselect a server
     */
    deselectServer: (serverId: string) => {
      set(state => {
        state.selectedServers.delete(serverId)

        // Update conversation config (or pending config)
        const configKey = state.currentConversationId || PENDING_CONVERSATION_KEY
        const config = state.conversationConfigs.get(configKey)
        if (config) {
          config.selectedServers.delete(serverId)
        }
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

        const newSelection = {
          server_id: serverId,
          tools: newTools,
        }

        state.selectedServers.set(serverId, newSelection)

        // Update conversation config (or pending config)
        const configKey = state.currentConversationId || PENDING_CONVERSATION_KEY
        const config = state.conversationConfigs.get(configKey)
        if (config) {
          config.selectedServers.set(serverId, newSelection)
        }
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

    /**
     * Set enabled servers from a list of IDs
     * Deselects all current servers, then selects only the provided IDs
     */
    setEnabledServers: (serverIds: string[]) => {
      set(state => {
        state.selectedServers.clear()
        for (const serverId of serverIds) {
          state.selectedServers.set(serverId, { server_id: serverId, tools: [] })
        }
      })
      console.log('[MCP Store] Set enabled servers:', serverIds)
    },

    // User defaults actions
    /**
     * Load user defaults from backend
     */
    loadUserDefaults: async () => {
      try {
        const { ApiClient } = await import('@/api-client')
        const response = await ApiClient.Mcp.getDefaults()
        set(state => {
          state.userDefaults = response.defaults || null
          state.userDefaultsLoaded = true
        })
        console.log('[MCP Store] Loaded user defaults:', response.defaults)
      } catch (error) {
        console.error('[MCP Store] Failed to load user defaults:', error)
        set(state => {
          state.userDefaultsLoaded = true
        })
      }
    },

    /**
     * Save current config as user defaults
     * availableServerIds is used to compute disabled_servers from selectedServers
     */
    saveUserDefaults: async (conversationId: string | null, availableServerIds: string[], updateAutoApproved?: boolean) => {
      const state = get()
      const configKey = conversationId || PENDING_CONVERSATION_KEY
      const config = state.conversationConfigs.get(configKey)

      // Use state.selectedServers directly (always available)
      // Config might not exist for new conversations, but selectedServers is always in state
      const selectedServerIds = new Set(state.selectedServers.keys())

      // Compute disabled_servers from selectedServers (inverted logic)
      // Any server NOT in selectedServers should be in disabled_servers
      const disabledServers: DisabledServer[] = availableServerIds
        .filter(id => !selectedServerIds.has(id))
        .map(id => ({ server_id: id, tools: [] }))

      try {
        const { ApiClient } = await import('@/api-client')
        const response = await ApiClient.Mcp.updateDefaults({
          approval_mode: config?.approvalMode || 'manual_approve',
          // Only send auto_approved_tools when explicitly changing approvals — backend COALESCE preserves DB value otherwise
          ...(updateAutoApproved ? { auto_approved_tools: config?.autoApprovedTools || [] } : {}),
          disabled_servers: disabledServers,
          loop_settings: config?.loopSettings,
        })
        set(state => {
          state.userDefaults = response
        })
        console.log('[MCP Store] Saved user defaults:', response, {
          selectedServers: selectedServerIds.size,
          disabledServers: disabledServers.length,
        })
      } catch (error) {
        console.error('[MCP Store] Failed to save user defaults:', error)
        throw error
      }
    },

    /**
     * Apply user defaults to pending config (for new conversations)
     */
    applyUserDefaultsToPending: (availableServerIds: string[]) => {
      const state = get()
      const defaults = state.userDefaults

      if (!defaults) {
        console.log('[MCP Store] No user defaults to apply')
        return
      }

      // Compute selected servers from disabled_servers
      // All available servers are selected EXCEPT those in disabled_servers
      const disabledServerIds = new Set((defaults.disabled_servers || []).map(d => d.server_id))
      const selectedServers = new Map<string, ServerSelection>()

      for (const serverId of availableServerIds) {
        if (!disabledServerIds.has(serverId)) {
          selectedServers.set(serverId, { server_id: serverId, tools: [] })
        }
      }

      set(s => {
        s.conversationConfigs.set(PENDING_CONVERSATION_KEY, {
          selectedServers,
          disabledServers: defaults.disabled_servers || [],
          approvalMode: defaults.approval_mode as 'disabled' | 'auto_approve' | 'manual_approve',
          autoApprovedTools: defaults.auto_approved_tools || [],
          loopSettings: defaults.loop_settings,
        })
        // Also update selectedServers if we're on a new conversation
        if (!s.currentConversationId) {
          s.selectedServers = new Map(selectedServers)
        }
      })
      console.log('[MCP Store] Applied user defaults to pending config:', {
        selectedServers: selectedServers.size,
        approvalMode: defaults.approval_mode,
      })
    },

    // Config modal actions
    /**
     * Open the config modal
     */
    openConfigModal: () => {
      set(state => {
        state.configModalVisible = true
      })
    },

    /**
     * Close the config modal
     */
    closeConfigModal: () => {
      set(state => {
        state.configModalVisible = false
      })
    },

    // Elicitation actions
    addElicitationRequest: (request: SSEChatStreamMcpElicitationRequiredData) => {
      set(state => {
        state.elicitationRequests.set(request.elicitation_id, {
          ...request,
          status: 'pending',
        })
      })
    },

    resolveElicitation: async (elicitation_id: string, action: 'accept' | 'decline' | 'cancel', content?: Record<string, unknown>) => {
      const finalStatus = action === 'accept' ? 'accepted' : action === 'decline' ? 'declined' : 'cancelled'

      // Optimistic update: set resolved status and response content so the component
      // renders the final state immediately without waiting for the API round-trip
      set(state => {
        const req = state.elicitationRequests.get(elicitation_id)
        if (req) {
          req.status = finalStatus
          if (action === 'accept' && content) {
            req.response_content = content
          }
        }
      })

      try {
        await ApiClient.Mcp.respondToElicitation({
          elicitation_id,
          action,
          ...(action === 'accept' && content ? { content } : {}),
        })
      } catch (e: unknown) {
        // 404 = session expired (MCP task already finished or page was reloaded)
        const is404 = e != null &&
          typeof e === 'object' &&
          'status' in e &&
          (e as { status: number }).status === 404
        if (is404) {
          set(state => {
            const req = state.elicitationRequests.get(elicitation_id)
            if (req) req.status = 'cancelled'
          })
        } else {
          console.error('[MCP Store] Failed to POST elicitation response:', e)
        }
      }
      // Note: we intentionally do NOT delete the entry — the component reads from McpStore
      // as the live source of truth during streaming. On page reload, the DB content
      // (with the persisted status) is used as the fallback.
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

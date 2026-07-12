import { enableMapSet } from 'immer'
import { defineStore } from '@/core/store-kit'
import { Permissions, type ToolApprovalDecision, type McpServerConfig, type AutoApprovedServer, type DisabledServer, type UserMcpDefaultsResponse, type LoopSettings, type ToolIdentifier, type PerToolLimit, type SSEChatStreamMcpElicitationRequiredData } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import {
  PENDING_CONVERSATION_KEY,
  pendingConversationKey,
  addApprovalDecisionTo,
  getApprovalDecisionsFrom,
  clearApprovalDecisionsIn,
} from '@/modules/mcp/stores/approvalRouting'

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
 * Live progress for a long-running tool call (from MCP
 * `notifications/progress` — e.g. a sandbox rootfs download).
 */
export interface McpToolProgressState {
  /** Current progress value (monotonically increasing). */
  progress: number
  /** Total expected units, if known (denominator for a progress bar). */
  total?: number | null
  /** Human-readable phase message ("Downloading…", "Verifying…"). */
  message?: string | null
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
  /** The streaming message this call belongs to (ITEM-33) — correlates a
   *  `notifications/progress` (server + message_id, no tool_use_id) to the RIGHT
   *  pane's call, so two panes running a tool on the same server don't cross-bleed. */
  message_id?: string
  tool_name: string
  status: 'started' | 'pending_approval' | 'completed' | 'error'
  input?: unknown
  result?: unknown
  error?: string
  /** Latest progress notification, while the call is running. */
  progress?: McpToolProgressState
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
// Per-conversation approval routing (ITEM-33) lives in a pure, enum-free module
// (imported at the top) so it can be unit-tested without importing this store.
// Re-exported so existing `PENDING_CONVERSATION_KEY` / `approvalKeyOf` importers
// are unchanged.
export {
  PENDING_CONVERSATION_KEY,
  pendingConversationKey,
  approvalKeyOf,
} from '@/modules/mcp/stores/approvalRouting'

/** Build a config-map key for a project (so the same conversationConfigs
 *  Map can hold both conversation- and project-scoped configs without a
 *  parallel collection). Conversation ids are raw UUIDs; project keys
 *  carry a `project:` prefix so they can't collide. */
export const projectConfigKey = (projectId: string) => `project:${projectId}`

/**
 * Resolve which config-map key an action should read/write based on
 * the currently-active scope (chat vs project). Centralizing this
 * keeps the precedence rule honest at every call site:
 *
 *   1. If `currentProjectId` is set AND `currentConversationId` is null,
 *      the modal was opened in project scope — route to the project key.
 *   2. Otherwise, route to the conversation id (or PENDING_CONVERSATION_KEY
 *      when null, i.e. the pending-buffer for new chats).
 *
 * Conversation always wins when present — even on a conversation that
 * belongs to a project. Editing from the chat surface edits conversation
 * overrides, never project defaults.
 */
function resolveConfigKey(
  state: {
    currentProjectId: string | null
    currentConversationId: string | null
    currentPaneId?: string | null
  },
  conversationId: string | null,
): string {
  if (state.currentProjectId !== null && state.currentConversationId === null) {
    return projectConfigKey(state.currentProjectId)
  }
  // No conversation → THIS pane's pending config (ITEM-51), so two new-chat split
  // panes edit their OWN pending config, not a single shared one.
  return conversationId || pendingConversationKey(state.currentPaneId)
}

/**
 * MCP extension store
 * Combines state and actions
 */

/**
 * MCP composer store — the chat composer's MCP server selections,
 * pending tool-call state, approval decisions, and elicitation requests.
 * Lives at `Stores.McpComposer` (registered in `modules/mcp/module.tsx`).
 * Prior name: `Stores.McpComposer`; relocated out of the chat-extension
 * framework so MCP-domain state lives in the MCP module.
 */
export const McpComposer = defineStore('McpComposer', {
  immer: true,
  state: {
    toolCalls: new Map<string, McpToolCall>(),
    // Per-conversation approval decisions (ITEM-33): keyed by approvalKeyOf so a
    // pane's approval is sent only with ITS conversation's next message.
    approvalDecisions: new Map<string, ToolApprovalDecision[]>(),
    conversationConfigs: new Map<string, ConversationMcpConfig>(),
    currentConversationId: null as string | null,
    // The pane the config modal / active selection is currently bound to (ITEM-51).
    // Only meaningful while `currentConversationId` is null (a new chat): it selects
    // WHICH pane's per-pane pending config the modal edits. null = single-pane.
    currentPaneId: null as string | null,
    currentProjectId: null as string | null,
    selectedServers: new Map<string, ServerSelection>(),
    userDefaults: null as UserMcpDefaultsResponse | null,
    userDefaultsLoaded: false,
    configModalVisible: false,
    elicitationRequests: new Map<string, ElicitationRequestState>(),
  },
  actions: (set, get) => ({

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
     * Attach progress to the running ('started') tool call(s) for a server.
     * `notifications/progress` carry server + message_id but not tool_use_id,
     * so we correlate to the in-flight call(s) from that server (typically
     * the single running execute_command download).
     */
    setToolCallProgress: (
      server: string,
      messageId: string | undefined,
      progress: McpToolProgressState,
    ) => {
      set(state => {
        for (const [id, call] of state.toolCalls) {
          // Match server AND the owning streaming message (ITEM-33/48), so a
          // progress event only updates the pane whose message spawned the call.
          // BUT a call is stamped with `streamingMessage?.id`, which may be a
          // synthetic client placeholder (`streaming-<ts>`, see Chat.store
          // placeholderId) that will NEVER equal the real server message_id a
          // progress event carries — so only a REAL (non-placeholder) call id is a
          // usable discriminator. When either side lacks a usable id, fall back to
          // server-only (the pre-split behaviour) so the progress bar never
          // silently stalls; the message_id refinement then only kicks in to avoid
          // cross-pane cross-talk when a real id IS available on both sides.
          const callMsgId = call.message_id
          const usableCallId = !!callMsgId && !callMsgId.startsWith('streaming-')
          const messageMatch = !messageId || !usableCallId || callMsgId === messageId
          if (call.server === server && messageMatch && call.status === 'started') {
            state.toolCalls.set(id, { ...call, progress })
          }
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
    addApprovalDecision: (
      conversationKey: string,
      decision: ToolApprovalDecision,
    ) => {
      set(state => {
        state.approvalDecisions = addApprovalDecisionTo(
          state.approvalDecisions,
          conversationKey,
          decision,
        )
      })
      console.log(
        '[MCP Store] Added approval decision:',
        conversationKey,
        decision.decision,
        decision.tool_use_id,
      )
    },

    /**
     * Get the pending approval decisions for ONE conversation (ITEM-33).
     */
    getApprovalDecisions: (conversationKey: string): ToolApprovalDecision[] => {
      return getApprovalDecisionsFrom(get().approvalDecisions, conversationKey)
    },

    /**
     * Clear ONE conversation's approval decisions (after that conversation sends).
     */
    clearApprovalDecisions: (conversationKey: string) => {
      set(state => {
        state.approvalDecisions = clearApprovalDecisionsIn(
          state.approvalDecisions,
          conversationKey,
        )
      })
      console.log('[MCP Store] Cleared approval decisions for', conversationKey)
    },

    // Conversation config actions
    /**
     * Set current conversation ID and load its config
     */
    setCurrentConversation: (conversationId: string | null, paneId?: string | null) => {
      set(state => {
        state.currentConversationId = conversationId
        // Bind the active selection / modal to the opening pane (ITEM-51) so a
        // new-chat toggle edits THAT pane's pending config, not a shared one.
        state.currentPaneId = paneId ?? null

        // Determine which config key to use (now paneId-aware for the pending case).
        const configKey = resolveConfigKey(state, conversationId)

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
          // THIS pane's pending config key (ITEM-51), not the single shared one.
          state.conversationConfigs.set(pendingConversationKey(paneId), pendingConfig)
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
     * Save the project's MCP defaults. Mirrors saveConversationConfig
     * (disabled_servers computed as the inverse of selectedServers
     * against availableServerIds + partial-tool disable from
     * serverToolsMap) but targets PUT /projects/{id}/mcp-settings.
     *
     * Kept as a separate action rather than a branch inside
     * saveConversationConfig — the two are conceptually different
     * (chat has the pending-buffer flow; project is direct) and
     * sharing the disabled-server derivation is a refactor we can
     * lift later if a third scope appears.
     */
    saveProjectConfig: async (
      projectId: string,
      availableServerIds?: string[],
      serverToolsMap?: Map<string, string[]>,
    ) => {
      const key = projectConfigKey(projectId)
      const state = get()
      const config = state.conversationConfigs.get(key)
      if (!config) {
        console.warn('[MCP Store] No project config to save for:', projectId)
        return
      }

      // Disabled-server derivation, identical to saveConversationConfig.
      let disabledServers: DisabledServer[] = []
      if (availableServerIds && availableServerIds.length > 0) {
        const selectedServerIds = new Set(config.selectedServers.keys())
        disabledServers = availableServerIds
          .filter(id => !selectedServerIds.has(id))
          .map(id => ({ server_id: id, tools: [] }))
      }
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
      const existingDisabled = config.disabledServers || []
      const availableSet = new Set(availableServerIds || [])
      const unavailableDisabled = existingDisabled.filter(d => !availableSet.has(d.server_id))
      disabledServers = [...disabledServers, ...unavailableDisabled]

      await ApiClient.Project.updateMcpSettings({
        id: projectId,
        approval_mode: config.approvalMode || 'manual_approve',
        auto_approved_tools: config.autoApprovedTools || [],
        disabled_servers: disabledServers,
        loop_settings: config.loopSettings,
      })

      set(state => {
        const existing = state.conversationConfigs.get(key)
        if (existing) {
          state.conversationConfigs.set(key, { ...existing, disabledServers })
        }
      })

      // Fire `project.mcp_updated` so the dedicated ProjectMcpSettings
      // store (used by the project panel) refetches and the UI reflects
      // the new defaults. Dynamic import to avoid module cycle with
      // @/core/stores.
      const { Stores } = await import('@/core/stores')
      await Stores.EventBus.emit({
        type: 'project.mcp_updated',
        data: { projectId },
      })

      console.log('[MCP Store] Saved project config:', projectId, {
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
    transferPendingConfig: (conversationId: string, paneId?: string | null) => {
      set(state => {
        // Move THIS pane's pending config to the freshly-minted conversation id
        // (ITEM-51): the sending pane's own pending key, not the shared one.
        const pendingKey = pendingConversationKey(paneId)
        const pendingConfig = state.conversationConfigs.get(pendingKey)
        if (pendingConfig) {
          // Copy pending config to new conversation
          state.conversationConfigs.set(conversationId, {
            ...pendingConfig,
            selectedServers: new Map(pendingConfig.selectedServers),
          })
          // Clear this pane's pending config
          state.conversationConfigs.delete(pendingKey)
          console.log('[MCP Store] Transferred pending config to conversation:', conversationId)
        }
      })
    },

    /**
     * Set approval mode for a conversation (or pending if conversationId is null)
     */
    setApprovalMode: (conversationId: string | null, mode: 'disabled' | 'auto_approve' | 'manual_approve') => {
      set(state => {
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, conversationId)
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
      const configKey = resolveConfigKey(state, state.currentConversationId)

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
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, conversationId)
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
        const configKey = resolveConfigKey(state, state.currentConversationId)
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
        const configKey = resolveConfigKey(state, state.currentConversationId)
        const config = state.conversationConfigs.get(configKey)
        if (config) {
          config.selectedServers.delete(serverId)
        }
      })
      console.log('[MCP Store] Deselected server:', serverId)
    },

    /** Per-pane (ITEM-47): deselect a server from a SPECIFIC conversation's config
     *  rather than the single global-active one — so removing a chip in a
     *  non-focused split pane edits THAT pane's conversation, never the focused
     *  pane's. Still mirrors into the active `selectedServers` projection when the
     *  target IS the currently-active conversation (keeps the active pane in sync). */
    deselectServerForConversation: (
      conversationId: string | null,
      serverId: string,
      paneId?: string | null,
    ) => {
      set(state => {
        // For a new chat (no conversationId) target THIS pane's pending config
        // (ITEM-51), so removing a chip in one new-chat pane doesn't touch another.
        const key = conversationId ?? pendingConversationKey(paneId)
        const config = state.conversationConfigs.get(key)
        if (config) config.selectedServers.delete(serverId)
        if (resolveConfigKey(state, state.currentConversationId) === key) {
          state.selectedServers.delete(serverId)
        }
      })
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
        const configKey = resolveConfigKey(state, state.currentConversationId)
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
     * The selected-servers config for a SPECIFIC conversation (ITEM-33) —
     * resolved from the per-conversation `conversationConfigs` (keyed), NOT the
     * single-active `selectedServers`. This is what the send path uses so two
     * split panes each send with THEIR OWN MCP server selection instead of
     * collapsing to whichever pane loaded last. Falls back to the active
     * `selectedServers` when the conversation has no stored config yet (the
     * just-created / same-conversation case).
     */
    getSelectedServersConfigFor: (
      conversationId: string | null | undefined,
    ): McpServerConfig[] => {
      const state = get()
      const key = conversationId ?? PENDING_CONVERSATION_KEY
      const config = state.conversationConfigs.get(key)
      const selections = config
        ? Array.from(config.selectedServers.values())
        : conversationId === state.currentConversationId
          ? Array.from(state.selectedServers.values())
          : []
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
      // Permission-gate the shell-eager-load fetch (audit follow-up):
      // /api/mcp/defaults is owned by the conversations module
      // (gated on ConversationsRead). The chat extensions panel
      // mounts even for users without that permission and the call
      // 403s otherwise.
      if (!hasPermissionNow(Permissions.ConversationsRead)) return

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
      const configKey = resolveConfigKey(state, conversationId)
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
        // Conversation-scoped open: clear any stale project scope so
        // the dispatch rule routes the save to the conversation path.
        state.currentProjectId = null
        state.configModalVisible = true
      })
    },

    /**
     * Open the config modal in PROJECT scope. Seeds a config under
     * `projectConfigKey(projectId)` from the supplied settings (fetched
     * upstream via `Stores.ProjectMcpSettings.loadSettings`) and clears
     * currentConversationId so the save dispatch routes to
     * /projects/{id}/mcp-settings. `settings` may be null when the
     * project has never customized defaults — we fall back to
     * manual_approve + empty arrays.
     */
    openConfigModalForProject: (
      projectId: string,
      settings:
        | import('@/modules/mcp/project-extension/stores/ProjectMcpSettings.store').ProjectMcpSettings
        | null,
    ) => {
      const key = projectConfigKey(projectId)
      set(state => {
        const autoApprovedRaw = (settings?.auto_approved_tools as
          | AutoApprovedServer[]
          | undefined) ?? []
        const disabledRaw = (settings?.disabled_servers as
          | DisabledServer[]
          | undefined) ?? []
        const loop = (settings?.loop_settings as
          | LoopSettings
          | null
          | undefined) ?? undefined

        // selectedServers stays empty here; the modal computes per-server
        // selection from disabledRaw + the live enabled-server list it
        // already loads (`selection = !disabledServers.find(...)`).
        const selectedServers = new Map<string, ServerSelection>()

        state.conversationConfigs.set(key, {
          selectedServers,
          disabledServers: disabledRaw,
          approvalMode: (settings?.approval_mode as
            | 'disabled'
            | 'auto_approve'
            | 'manual_approve') || 'manual_approve',
          autoApprovedTools: autoApprovedRaw,
          loopSettings: loop,
        })

        // RESET the GLOBAL `selectedServers` Map. This is the single
        // top-level state field used by the modal as its working copy
        // (mutated by selectServer / toggleServerTool / etc). Without
        // this reset, stale entries from a previous modal session —
        // typically a chat conversation's MCP config loaded via
        // setCurrentConversation — survive across modal opens and
        // trip the seed-once guard in McpConfigModal.tsx:
        //
        //     if (selectedServers.size > 0) return   // skip seeding
        //
        // The guard's intent is to preserve mid-edit toggles, NOT to
        // preserve state across opens — but the Map lives at the store
        // root, so the two cases conflate. Clearing here makes every
        // open of the project modal start from a known-empty state;
        // the seed then runs and computes the toggle state from
        // disabled_servers + the enabledServers list, which is what
        // makes "Web Fetch disabled" actually show as disabled after a
        // reload.
        state.selectedServers = new Map()

        state.currentProjectId = projectId
        state.currentConversationId = null
        state.configModalVisible = true
      })
    },

    /**
     * Close the config modal. Clears project scope so reopening from
     * chat doesn't accidentally route via stale state.
     */
    closeConfigModal: () => {
      set(state => {
        state.configModalVisible = false
        state.currentProjectId = null
      })
    },

    listServerTools: async (serverId: string) => {
      return await ApiClient.McpServerRuntime.listTools({ id: serverId })
    },

    getConversationMcpSettings: async (conversationId: string) => {
      return await ApiClient.Conversation.getMcpSettings({
        id: conversationId,
      })
    },

    getBranchPendingApprovals: async (branchId: string) => {
      return await ApiClient.Branch.getPendingApprovals({
        branch_id: branchId,
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
        const status = e != null && typeof e === 'object' && 'status' in e
          ? (e as { status?: number }).status
          : undefined
        set(state => {
          const req = state.elicitationRequests.get(elicitation_id)
          if (!req) return
          // 404 = the elicitation is already gone server-side (the MCP task
          // finished or the page was reloaded) → treat it as cancelled. On any
          // other failure (network, 400, 403 owner-bind race, 5xx) the server
          // did NOT record our answer, so roll the optimistic flip back to
          // 'pending' so the user can retry instead of seeing a false success.
          req.status = status === 404 ? 'cancelled' : 'pending'
        })
        if (status !== 404) {
          console.error('[MCP Store] Failed to POST elicitation response:', e)
        }
      }
      // Note: we intentionally do NOT delete the entry — the component reads from McpComposer
      // as the live source of truth during streaming. On page reload, the DB content
      // (with the persisted status) is used as the fallback.
    },
  }),
  init: ({ on, actions }) => {
    // Cross-device sync of the user's own MCP defaults. `loadUserDefaults`
    // self-gates on `conversations::read` internally (returns early when the
    // user lacks it), satisfying the no-403 reconnect rule — `sync:reconnect`
    // fires for every store regardless of audience.
    const reload = () => void actions.loadUserDefaults()
    on('sync:mcp_defaults', reload)
    on('sync:reconnect', reload)
    void actions.loadUserDefaults()
  },
})

export const useMcpComposerStore = McpComposer.store

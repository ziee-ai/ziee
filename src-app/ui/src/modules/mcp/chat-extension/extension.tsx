import { useState } from 'react'
import { Alert, Button, Card, Progress, Text } from '@/components/ui'
import { Wrench, CircleCheck, CircleX } from 'lucide-react'
import {
  createExtension,
  type ChatExtension,
  type ContentRendererProps,
} from '@/modules/chat/core/extensions'
import { Stores } from '@/core/stores'
import type { McpToolCall } from '@/modules/mcp/stores/McpComposer.store'
import type { MessageContent, MessageContentDataToolUse, MessageContentDataToolResult, MessageWithContent } from '@/api-client/types'
import { ToolCallPendingApprovalContent } from '@/modules/mcp/chat-extension/components/ToolCallPendingApprovalContent'
import { McpMenuItem } from '@/modules/mcp/chat-extension/components/McpMenuItem'
import { McpStatusRow } from '@/modules/mcp/chat-extension/components/McpStatusRow'
import { McpInitializer } from '@/modules/mcp/chat-extension/components/McpInitializer'
import { ElicitationFormContent } from '@/modules/mcp/chat-extension/components/ElicitationFormContent'

/**
 * MCP Tool Call UI Component
 * Shows approval UI when status is 'pending_approval'
 */
function McpToolCallUI({ toolCall }: { toolCall: McpToolCall }) {
  const [isExpanded, setIsExpanded] = useState(false)

  // Show approval UI for pending approval status
  if (toolCall.status === 'pending_approval') {
    return <ToolCallPendingApprovalContent toolCall={toolCall} />
  }

  const getStatusIcon = () => {
    switch (toolCall.status) {
      case 'started':
        return <Wrench className="text-blue-500 animate-spin" />
      case 'completed':
        return <CircleCheck className="text-green-500" />
      case 'error':
        return <CircleX className="text-red-500" />
    }
  }

  const getStatusText = () => {
    switch (toolCall.status) {
      case 'started':
        return 'Running...'
      case 'completed':
        return 'Completed'
      case 'error':
        return 'Failed'
    }
  }

  return (
    <Card
      size="sm"
      className="mb-2 bg-black/2"
      data-testid={`mcp-toolcall-card-${toolCall.tool_use_id}`}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {getStatusIcon()}
          <Text strong>{toolCall.tool_name}</Text>
          <Text type="secondary" className="text-xs">
            ({toolCall.server})
          </Text>
          <Text type="secondary" className="text-xs">
            {getStatusText()}
          </Text>
        </div>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => setIsExpanded(!isExpanded)}
          data-testid={`mcp-toolcall-details-btn-${toolCall.tool_use_id}`}
        >
          {isExpanded ? 'Hide' : 'Show'} details
        </Button>
      </div>

      {toolCall.status === 'started' && toolCall.progress && (
        <div className="mt-2">
          {toolCall.progress.message && (
            <Text type="secondary" className="text-xs">
              {toolCall.progress.message}
            </Text>
          )}
          <Progress
            size="sm"
            aria-label="Tool call progress"
            data-testid={`mcp-toolcall-progress-${toolCall.tool_use_id}`}
            value={
              toolCall.progress.total && toolCall.progress.total > 0
                ? Math.min(
                    100,
                    Math.round(
                      (toolCall.progress.progress / toolCall.progress.total) * 100,
                    ),
                  )
                : 0
            }
          />
        </div>
      )}

      {isExpanded && (
        <div className="mt-2 text-xs">
          {toolCall.input !== undefined && (
            <div className="mb-2">
              <Text strong>Input:</Text>
              <pre className="p-2 rounded mt-1 overflow-auto max-h-40">
                {JSON.stringify(toolCall.input, null, 2)}
              </pre>
            </div>
          )}

          {toolCall.result !== undefined && (
            <div className="mb-2">
              <Text strong>Result:</Text>
              <pre className="p-2 rounded mt-1 overflow-auto max-h-40">
                {JSON.stringify(toolCall.result, null, 2)}
              </pre>
            </div>
          )}

          {toolCall.error && (
            <Alert
              tone="error"
              title="Error"
              description={toolCall.error}
              data-testid={`mcp-toolcall-error-alert-${toolCall.tool_use_id}`}
            />
          )}
        </div>
      )}
    </Card>
  )
}

/**
 * MCP tool use content renderer component
 * Renders tool calls from MCP servers (the call itself, before result)
 */
function McpToolUseRenderer({ content: data }: ContentRendererProps) {
  const [isExpanded, setIsExpanded] = useState(false)
  // Access toolCalls Map directly to create a reactive subscription
  // Using getToolCall() method doesn't trigger re-renders when store updates
  const { toolCalls } = Stores.McpComposer
  const { servers } = Stores.McpServer
  const toolUseData = data.content as MessageContentDataToolUse

  if (!toolUseData.id) {
    return null
  }

  const toolCall = toolCalls.get(toolUseData.id)

  // If we have a tracked tool call, render it
  if (toolCall) {
    return <McpToolCallUI toolCall={toolCall} />
  }

  // Look up server name from server_id
  const server = servers.find(s => s.id === toolUseData.server_id)
  const serverName = server?.display_name || toolUseData.server_id || 'Unknown'

  // Look up matching tool_result for historical display
  const message = Stores.Chat.messages.get(data.message_id)
  const toolResultData = message?.contents.find(
    c =>
      c.content_type === 'tool_result' &&
      ((c.content as unknown as { tool_use_id: string }).tool_use_id === toolUseData.id),
  )?.content as unknown as { content: string; is_error?: boolean } | undefined

  const hasDetails = toolUseData.input || toolResultData

  // Historical view for tool calls loaded from DB (store is empty after reload)
  return (
    <Card size="sm" className="mb-2 bg-black/2" data-testid={`mcp-tooluse-card-${toolUseData.id}`}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {toolResultData?.is_error ? (
            <CircleX className="text-red-500" />
          ) : toolResultData ? (
            <CircleCheck className="text-green-500" />
          ) : (
            <Wrench className="text-blue-500" />
          )}
          <Text strong>{toolUseData.name || 'Tool Call'}</Text>
          <Text type="secondary" className="text-xs">({serverName})</Text>
          {toolResultData && (
            <Text type="secondary" className="text-xs">
              {toolResultData.is_error ? 'Failed' : 'Completed'}
            </Text>
          )}
        </div>
        {hasDetails && (
          <Button size="sm" variant="ghost" onClick={() => setIsExpanded(!isExpanded)} data-testid={`mcp-tooluse-details-btn-${toolUseData.id}`}>
            {isExpanded ? 'Hide' : 'Show'} details
          </Button>
        )}
      </div>
      {isExpanded && (
        <div className="mt-2 text-xs">
          {!!toolUseData.input && (
            <div className="mb-2">
              <Text strong>Input:</Text>
              <pre className="p-2 rounded mt-1 overflow-auto max-h-40">
                {JSON.stringify(toolUseData.input, null, 2)}
              </pre>
            </div>
          )}
          {toolResultData && (
            <div className="mb-2">
              <Text strong>Result:</Text>
              {toolResultData.is_error ? (
                <Alert tone="error" title="Error" description={toolResultData.content} className="mt-1" data-testid={`mcp-tooluse-error-alert-${toolUseData.id}`} />
              ) : (
                <pre className="p-2 rounded mt-1 overflow-auto max-h-40">{toolResultData.content}</pre>
              )}
            </div>
          )}
        </div>
      )}
    </Card>
  )
}

/**
 * MCP Extension
 * Handles MCP tool calls, approval workflows, and renders tool call UI
 */
const mcpExtension: ChatExtension = createExtension({
  name: 'mcp',
  description: 'Handles MCP tool calls and approval workflows',
  priority: 50, // Higher priority to handle events early

  initialize: async () => {
    console.log('[MCP Extension] Initialized')

    const { useChatStore } = await import('@/modules/chat/core/stores/Chat.store')
    const { Stores } = await import('@/core/stores')
    const { ApiClient } = await import('@/api-client')

    useChatStore.subscribe(
      state => state.editingMessage,
      async (editingMessage) => {
        const mcpStore = Stores.McpComposer
        if (!mcpStore) return

        if (editingMessage) {
          // Per-message server snapshot moved off the Message row into
          // mcp's own `message_mcp_servers` join table (backend
          // migration 74). Fetch via the mcp-owned endpoint instead of
          // reading inline from `editingMessage.mcp_server_ids` (which
          // no longer exists on the Message type).
          try {
            const resp = await ApiClient.Message.getMcpServers({
              id: editingMessage.id,
            })
            if (resp.server_ids.length > 0) {
              mcpStore.setEnabledServers(resp.server_ids)
            }
          } catch (err) {
            // Soft-fail: no snapshot recorded (pre-migration message
            // or write hook failed at send-time) → keep current
            // selection. Matches the pre-extraction behavior for
            // messages without the column populated.
            console.warn(
              '[MCP Extension] Failed to load message server snapshot:',
              err,
            )
          }
        } else {
          // Edit cancelled or sent — restore from stored conversation config
          const conversation = useChatStore.getState().conversation
          if (conversation) {
            mcpStore.setCurrentConversation(conversation.id)
          }
        }
      }
    )
  },

  // Type-safe SSE event handlers
  sseEventHandlers: {
    mcpToolStart: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamMcpToolStartData
      // Access store via __state to avoid triggering React hooks outside component context
      const mcpStore = Stores.McpComposer

      mcpStore.addToolCall({
        tool_use_id: data.tool_use_id,
        server: data.server,
        tool_name: data.tool_name,
        status: 'started',
        input: data.input,
      })

      // Inject tool_use content block into streaming message so McpToolUseRenderer can mount
      // This ensures tool calls are visible during auto-approve execution
      const chatState = get()
      let streamingMessage = chatState.streamingMessage
      const now = new Date().toISOString()

      // Create tool_use content block
      const toolUseContent: MessageContent = {
        id: '',
        message_id: '',
        content_type: 'tool_use',
        content: {
          type: 'tool_use',
          id: data.tool_use_id,
          name: data.tool_name,
          server_id: data.server,
        } as MessageContentDataToolUse,
        sequence_order: 0,
        created_at: now,
        updated_at: now,
      }

      if (streamingMessage) {
        // Check if this tool_use content already exists (avoid duplicates)
        const exists = streamingMessage.contents.some(
          c => c.content_type === 'tool_use' &&
               (c.content as MessageContentDataToolUse).id === data.tool_use_id
        )
        if (!exists) {
          toolUseContent.id = `${streamingMessage.id}-tool-${data.tool_use_id}`
          toolUseContent.message_id = streamingMessage.id
          toolUseContent.sequence_order = streamingMessage.contents.length

          const updatedMessage = {
            ...streamingMessage,
            contents: [...streamingMessage.contents, toolUseContent],
          }

          const newMessages = new Map(chatState.messages)
          newMessages.set(updatedMessage.id, updatedMessage)
          set({
            streamingMessage: updatedMessage,
            messages: newMessages,
          })
        }
      } else {
        // No streaming message exists — check messages map for an existing block first (dedup)
        const existingInMap = [...chatState.messages.values()].some(m =>
          m.contents.some(
            c => c.content_type === 'tool_use' &&
                 (c.content as MessageContentDataToolUse).id === data.tool_use_id
          )
        )

        if (!existingInMap) {
          // CREATE a new streaming message with the tool_use block
          const messageId = `streaming-${Date.now()}`
          toolUseContent.id = `${messageId}-tool-${data.tool_use_id}`
          toolUseContent.message_id = messageId

          const newMessage: MessageWithContent = {
            id: messageId,
            role: 'assistant',
            contents: [toolUseContent],
            originated_from_id: '',
            edit_count: 0,
            created_at: now,
          }

          const newMessages = new Map(chatState.messages)
          newMessages.set(newMessage.id, newMessage)
          set({
            streamingMessage: newMessage,
            messages: newMessages,
          })
        }
      }

      console.log('[MCP Extension] Tool started:', data.tool_name)
    },

    mcpToolProgress: async data => {
      // A long-running tool call reported progress (e.g. a sandbox rootfs
      // download). Attach it to the running tool call(s) for this server so
      // the tool card can render a live progress bar.
      const mcpStore = Stores.McpComposer
      mcpStore.setToolCallProgress(data.server, {
        progress: data.progress,
        total: data.total ?? undefined,
        message: data.message ?? undefined,
      })
    },

    mcpApprovalRequired: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamMcpApprovalRequiredData
      const mcpStore = Stores.McpComposer

      // Use addToolCall instead of updateToolCall - the tool call doesn't exist yet
      // because mcpToolStart is NOT sent when approval is required
      mcpStore.addToolCall({
        tool_use_id: data.tool_use_id,
        server: data.server,
        server_id: data.server_id,
        tool_name: data.tool_name,
        status: 'pending_approval',
        input: data.input,
      })

      // Inject tool_use content block into streaming message so McpToolUseRenderer can mount
      // Without this, the approval UI won't show because there's no content block to render
      const chatState = get()
      let streamingMessage = chatState.streamingMessage
      const now = new Date().toISOString()

      // Create tool_use content block
      const toolUseContent: MessageContent = {
        id: '', // Will be set below based on message id
        message_id: '', // Will be set below
        content_type: 'tool_use',
        content: {
          type: 'tool_use',
          id: data.tool_use_id,
          name: data.tool_name,
          server_id: data.server_id,
          input: data.input,
        } as MessageContentDataToolUse,
        sequence_order: 0,
        created_at: now,
        updated_at: now,
      }

      if (streamingMessage) {
        // Streaming message exists - add tool_use content to it (dedup check)
        const existsInStreaming = streamingMessage.contents.some(
          c => c.content_type === 'tool_use' &&
               (c.content as MessageContentDataToolUse).id === data.tool_use_id
        )
        // Also check the full messages map: on approval-resend streams, asst_msg_1 may
        // already be loaded via loadMessages and contain this tool_use_id. Without this
        // check the handler would add a second tool_use block to the new streaming message,
        // producing two McpToolUseRenderer instances → two approval panels.
        const existsInMap = !existsInStreaming && [...chatState.messages.values()].some(m =>
          m.id !== streamingMessage.id &&
          m.contents.some(
            c => c.content_type === 'tool_use' &&
                 (c.content as MessageContentDataToolUse).id === data.tool_use_id
          )
        )
        if (!existsInStreaming && !existsInMap) {
          toolUseContent.id = `${streamingMessage.id}-tool-${data.tool_use_id}`
          toolUseContent.message_id = streamingMessage.id
          toolUseContent.sequence_order = streamingMessage.contents.length

          const updatedMessage = {
            ...streamingMessage,
            contents: [...streamingMessage.contents, toolUseContent],
          }

          const newMessages = new Map(chatState.messages)
          newMessages.set(updatedMessage.id, updatedMessage)
          set({
            streamingMessage: updatedMessage,
            messages: newMessages,
          })

          console.log('[MCP Extension] Added tool_use content block to existing streaming message:', data.tool_name)
        }
      } else {
        // No streaming message exists — check messages map for an existing one first (dedup)
        const existingStreaming = [...chatState.messages.values()].find(m =>
          m.contents.some(
            c => c.content_type === 'tool_use' &&
                 (c.content as MessageContentDataToolUse).id === data.tool_use_id
          )
        )

        if (!existingStreaming) {
          // CREATE a new streaming message with the tool_use block
          // This happens when LLM returns a tool call without any text first
          const messageId = `streaming-${Date.now()}`
          toolUseContent.id = `${messageId}-tool-${data.tool_use_id}`
          toolUseContent.message_id = messageId

          const newMessage: MessageWithContent = {
            id: messageId,
            role: 'assistant',
            contents: [toolUseContent],
            originated_from_id: '',
            edit_count: 0,
            created_at: now,
          }

          const newMessages = new Map(chatState.messages)
          newMessages.set(newMessage.id, newMessage)
          set({
            streamingMessage: newMessage,
            messages: newMessages,
          })

          console.log('[MCP Extension] Created new streaming message with tool_use block:', data.tool_name)
        }
      }

      console.log('[MCP Extension] Approval required for:', data.tool_name)
    },

    mcpElicitationRequired: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamMcpElicitationRequiredData
      const mcpStore = Stores.McpComposer
      mcpStore.addElicitationRequest(data)

      // Inject elicitation_request content block into streaming message so
      // ElicitationFormContent can mount and render the form inline
      const chatState = get()
      const streamingMessage = chatState.streamingMessage
      const now = new Date().toISOString()

      const elicitContent = {
        id: '',
        message_id: '',
        content_type: 'elicitation_request',
        content: {
          type: 'elicitation_request',
          status: 'pending',
          elicitation_id: data.elicitation_id,
          message_id: data.message_id,
          message: data.message,
          requested_schema: data.requested_schema,
          server: data.server,
        },
        sequence_order: 0,
        created_at: now,
        updated_at: now,
      } as unknown as MessageContent

      if (streamingMessage) {
        // Dedup: don't inject the same elicitation block twice (keyed by unique elicitation_id)
        const exists = streamingMessage.contents.some(
          c =>
            c.content_type === 'elicitation_request' &&
            (c.content as unknown as { elicitation_id: string }).elicitation_id === data.elicitation_id,
        )
        if (!exists) {
          elicitContent.id = `${streamingMessage.id}-elicit-${data.elicitation_id}`
          elicitContent.message_id = streamingMessage.id
          elicitContent.sequence_order = streamingMessage.contents.length

          const updatedMessage = {
            ...streamingMessage,
            contents: [...streamingMessage.contents, elicitContent],
          }

          const newMessages = new Map(chatState.messages)
          newMessages.set(updatedMessage.id, updatedMessage)
          set({ streamingMessage: updatedMessage, messages: newMessages })
        }
      } else {
        // No streaming message — create one to host the form
        const messageId = `streaming-${Date.now()}`
        elicitContent.id = `${messageId}-elicit-${data.elicitation_id}`
        elicitContent.message_id = messageId

        const newMessage: MessageWithContent = {
          id: messageId,
          role: 'assistant',
          contents: [elicitContent],
          originated_from_id: '',
          edit_count: 0,
          created_at: now,
        }

        const newMessages = new Map(chatState.messages)
        newMessages.set(newMessage.id, newMessage)
        set({ streamingMessage: newMessage, messages: newMessages })
      }

      console.log('[MCP Extension] Elicitation required:', data.message_id, 'from', data.server)
    },

    mcpToolComplete: async (data, _get, _set) => {
      // data is automatically typed as SSEChatStreamMcpToolCompleteData
      const mcpStore = Stores.McpComposer

      mcpStore.updateToolCall(data.tool_use_id, {
        status: data.is_error ? 'error' : 'completed',
        error: data.is_error ? 'Tool execution failed' : undefined,
        result: data.result,
      })

      console.log(
        '[MCP Extension] Tool completed:',
        data.tool_use_id,
        data.is_error ? '(error)' : '(success)',
      )
    },

    artifactCreated: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamArtifactCreatedData.
      // A tool returned a file artifact. Surface it during streaming the SAME
      // way it persists in the DB: as a `resource_link` on the tool_result
      // block for the producing tool call. The file extension's `tool_result`
      // content renderer then shows the inline preview at that block's
      // position — consistent during streaming and after the post-complete
      // reload (no FileCard flash, no jump to a footer).
      const chatState = get()
      const streamingMessage = chatState.streamingMessage
      if (!streamingMessage) return

      // Associate to the producing tool call. Prefer the explicit tool_use_id
      // from the event (robust under parallel tools); fall back to the last
      // tool_use block for older backends that don't send it.
      let toolUseId = data.tool_use_id
      if (!toolUseId) {
        for (let i = streamingMessage.contents.length - 1; i >= 0; i--) {
          const c = streamingMessage.contents[i]
          if (c.content_type === 'tool_use') {
            toolUseId = (c.content as MessageContentDataToolUse).id
            break
          }
        }
      }
      if (!toolUseId) return

      // Backend-owned artifact: render via the authenticated `/api/files/{id}`
      // path. InlineFilePreview resolves the File entity by `file_id`, so this
      // synthetic uri is only the React/dedup key until the real link arrives
      // on reload.
      const link = {
        uri: `/api/files/${data.file_id}`,
        file_id: data.file_id,
        name: data.filename,
        mime_type: data.mime_type ?? undefined,
        size: data.file_size,
        is_saved: true,
      }

      const now = new Date().toISOString()
      const contents = [...streamingMessage.contents]
      const existingIdx = contents.findIndex(
        c =>
          c.content_type === 'tool_result' &&
          (c.content as unknown as MessageContentDataToolResult).tool_use_id === toolUseId,
      )

      if (existingIdx >= 0) {
        // Merge: a tool that produced several artifacts collects them into one
        // tool_result block. Dedupe by file_id so repeated events don't stack.
        const existing = contents[existingIdx]
        const existingData = existing.content as unknown as MessageContentDataToolResult
        const links = [...(existingData.resource_links ?? [])]
        if (!links.some(l => l.file_id === link.file_id)) {
          links.push(link)
        }
        contents[existingIdx] = {
          ...existing,
          content: { ...existingData, resource_links: links } as unknown as MessageContentDataToolResult,
        }
      } else {
        // First artifact for this tool: create the tool_result block right
        // after its tool_use (append → monotonic sequence_order). `content`
        // is empty — the result text is shown by the tool_use card; this block
        // only carries the files. `tool_use_id` lets the card's historical
        // lookup still match it after reload.
        const toolResult: MessageContent = {
          id: `artifact-result-${toolUseId}`,
          message_id: streamingMessage.id,
          content_type: 'tool_result',
          content: {
            type: 'tool_result',
            tool_use_id: toolUseId,
            content: '',
            resource_links: [link],
          } as unknown as MessageContentDataToolResult,
          sequence_order: contents.length,
          created_at: now,
          updated_at: now,
        }
        contents.push(toolResult)
      }

      const updatedMessage = { ...streamingMessage, contents }
      const newMessages = new Map(chatState.messages)
      newMessages.set(updatedMessage.id, updatedMessage)
      set({ streamingMessage: updatedMessage, messages: newMessages })

      console.log('[MCP Extension] Artifact created:', data.filename, data.file_id, '→ tool', toolUseId)
    },
  },

  // Allow empty text when there are pending tool approvals
  beforeSendMessage: async () => {
    const { Stores } = await import('@/core/stores')
    const mcpStore = Stores.McpComposer

    // Check if there are approval decisions queued to send
    const approvalDecisions = mcpStore.getApprovalDecisions()
    const hasApprovalDecisions = approvalDecisions.length > 0

    if (hasApprovalDecisions) {
      // Discard text extension's cancel since we're sending tool approvals
      console.log('[MCP Extension] Has approval decisions, discarding text cancel')
      return { cancel: false, discardCancel: ['text'] }
    }

    return { cancel: false }
  },

  // Compose request fields to include MCP config and approval decisions
  composeRequestFields: async () => {
    const { Stores } = await import('@/core/stores')
    const mcpStore = Stores.McpComposer
    const selectedServers = mcpStore.getSelectedServersConfig()
    const approvalDecisions = mcpStore.getApprovalDecisions()

    const fields: any = {}

    // Add MCP config if servers are selected
    if (selectedServers.length > 0) {
      fields.enable_mcp = true
      fields.mcp_config = { mcp_servers: selectedServers }
      console.log('[MCP Extension] Including MCP config:', fields.mcp_config)
    }

    // Add approval decisions if present
    if (approvalDecisions.length > 0) {
      fields.tool_approvals = approvalDecisions
      console.log('[MCP Extension] Including approval decisions:', approvalDecisions)
    }

    return fields
  },

  // Load conversation MCP settings when conversation is opened
  onConversationLoad: async (conversation) => {
    const { Stores } = await import('@/core/stores')
    const mcpStore = Stores.McpComposer
    const mcpStoreProxy = Stores.McpComposer

    // Set current conversation ID
    mcpStore.setCurrentConversation(conversation.id)

    try {
      // Load conversation MCP settings from backend (via store action).
      const response = await mcpStoreProxy.getConversationMcpSettings(
        conversation.id,
      )

      // Get available servers to compute selectedServers from disabledServers
      // Access __state directly on the McpServer store (outside React context)
      const mcpServerState = Stores.McpServer.__state
      const availableServers = (mcpServerState?.servers || []).filter(s => s.enabled)
      const availableServerIds = new Set(availableServers.map(s => s.id))

      if (response.settings) {
        // Get disabled servers from backend
        const disabledServers = response.settings.disabled_servers || []

        // Compute selectedServers: all available servers that are NOT fully disabled.
        // Entries with non-empty tools = partially disabled (specific tools disabled, server still enabled).
        const selectedServers = new Map<string, { server_id: string; tools: string[] }>()
        for (const serverId of availableServerIds) {
          const disabledEntry = disabledServers.find(d => d.server_id === serverId)

          if (!disabledEntry) {
            // Not in disabled list → all tools selected
            selectedServers.set(serverId, { server_id: serverId, tools: [] })
          } else if (disabledEntry.tools.length > 0) {
            // Partially disabled: specific tools are disabled, compute selected = all - disabled
            try {
              const toolsResponse = await mcpStoreProxy.listServerTools(serverId)
              const allTools = toolsResponse.tools.map(t => t.name)
              const selectedTools = allTools.filter(t => !disabledEntry.tools.includes(t))
              if (selectedTools.length > 0) {
                selectedServers.set(serverId, { server_id: serverId, tools: selectedTools })
              }
              // If all tools are disabled (selectedTools empty), treat server as disabled → skip
            } catch {
              // On error fetching tools, fall back to all tools selected
              selectedServers.set(serverId, { server_id: serverId, tools: [] })
            }
          }
          // disabledEntry.tools.length === 0 → entire server disabled → skip
        }

        const config = {
          selectedServers,
          disabledServers,
          approvalMode: response.settings.approval_mode as 'disabled' | 'auto_approve' | 'manual_approve',
          autoApprovedTools: response.settings.auto_approved_tools || [],
          loopSettings: response.settings.loop_settings,
        }

        mcpStore.loadConversationConfig(conversation.id, config)
        console.log('[MCP Extension] Loaded conversation MCP config:', conversation.id, {
          availableServers: availableServerIds.size,
          disabledServers: disabledServers.length,
          selectedServers: selectedServers.size,
          loopSettings: response.settings.loop_settings,
        })
      } else {
        // If settings don't exist yet, select all available servers by default
        const selectedServers = new Map<string, { server_id: string; tools: string[] }>()
        for (const serverId of availableServerIds) {
          selectedServers.set(serverId, { server_id: serverId, tools: [] })
        }

        const config = {
          selectedServers,
          disabledServers: [],
          approvalMode: 'manual_approve' as const,
          autoApprovedTools: [],
          loopSettings: undefined,  // Use defaults
        }

        mcpStore.loadConversationConfig(conversation.id, config)
        console.log('[MCP Extension] No existing config, using defaults with all servers enabled:', conversation.id)
      }
    } catch (error) {
      // If settings don't exist yet, create default config with all servers enabled
      const mcpServerState = Stores.McpServer.__state
      const availableServers = (mcpServerState?.servers || []).filter(s => s.enabled)
      const selectedServers = new Map<string, { server_id: string; tools: string[] }>()
      for (const server of availableServers) {
        selectedServers.set(server.id, { server_id: server.id, tools: [] })
      }

      const config = {
        selectedServers,
        disabledServers: [],
        approvalMode: 'manual_approve' as const,
        autoApprovedTools: [],
        loopSettings: undefined,  // Use defaults
      }

      mcpStore.loadConversationConfig(conversation.id, config)
      console.log('[MCP Extension] Error loading config, using defaults:', conversation.id, error)
    }

    // Load pending approvals for the current branch (to restore state after page refresh)
    if (conversation.active_branch_id) {
      try {
        const approvalsResponse = await mcpStoreProxy.getBranchPendingApprovals(
          conversation.active_branch_id,
        )

        if (approvalsResponse.approvals && approvalsResponse.approvals.length > 0) {
          for (const approval of approvalsResponse.approvals) {
            mcpStore.addToolCall({
              tool_use_id: approval.tool_use_id,
              server: approval.server_name,
              server_id: approval.server_id,
              tool_name: approval.tool_name,
              status: 'pending_approval',
              input: approval.input,
            })
          }
          console.log(
            '[MCP Extension] Loaded pending approvals:',
            approvalsResponse.approvals.length,
          )
        }
      } catch (error) {
        console.error('[MCP Extension] Failed to load pending approvals:', error)
      }
    }
  },

  // Clear approval decisions after message is sent
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    // Use __state on McpStore too since it's also a proxy
    const mcpStore = Stores.McpComposer.__state
    const chatStore = Stores.Chat.__state

    // Get current conversation from chat store
    const conversation = chatStore.conversation

    // Handle new conversation creation
    if (conversation?.id && !mcpStore.currentConversationId) {
      console.log('[MCP Extension] New conversation created, transferring pending config:', conversation.id)

      // Transfer pending config to the new conversation
      mcpStore.transferPendingConfig(conversation.id)

      // Set current conversation ID
      mcpStore.setCurrentConversation(conversation.id)

      // Get available server IDs for proper disabled_servers computation
      const mcpServerState = Stores.McpServer.__state
      const availableServerIds = (mcpServerState?.servers || [])
        .filter(s => s.enabled)
        .map(s => s.id)

      // Save settings to backend with available server IDs
      try {
        await mcpStore.saveConversationConfig(conversation.id, availableServerIds)
      } catch (error) {
        console.error('[MCP Extension] Failed to save config for new conversation:', error)
      }
    }

    mcpStore.clearApprovalDecisions()
    console.log('[MCP Extension] Cleared approval decisions after message sent')

    return {}
  },

  // Register content type components.
  // NOTE: `tool_result` is intentionally NOT registered here. The tool-call
  // CARD (input + result text, including completed/error state) is rendered by
  // McpToolUseRenderer (the `tool_use` content type). The file extension owns
  // the `tool_result` content type so a tool's returned files (resource_links)
  // render INLINE at that block's position. The registry returns the first
  // renderer for a content type, so registering a null renderer here would
  // shadow the file extension's.
  contentTypes: {
    tool_use: McpToolUseRenderer,
    elicitation_request: ElicitationFormContent,
  },

  // Register slot components
  slots: {
    toolbar_actions: { component: McpInitializer, order: 1 },
    toolbar_plus_items: { component: McpMenuItem, order: 20 },
    toolbar_status: { component: McpStatusRow, order: 10 },
  },

  cleanup: async () => {
    console.log('[MCP Extension] Cleaned up')
  },
})

export default mcpExtension

import { Fragment, useState } from 'react'
import { Alert, Button, Card, Progress, Text } from '@/components/ui'
import { ChevronDown } from 'lucide-react'
import { cn } from '@/lib/utils'
import { ToolStatusIcon } from '@/modules/chat/core/ToolStatusIcon'
import { mcpServerParenLabel } from '@/modules/mcp/chat-extension/serverLabel'
import {
  createExtension,
  chatExtensionRegistry,
  type ChatExtension,
  type ContentRendererProps,
} from '@/modules/chat/core/extensions'
import { Stores } from '@/core/stores'
import type { McpToolCall } from '@/modules/mcp/stores/McpComposer.store'
import type { MessageContent, MessageContentDataToolUse, MessageContentDataToolResult, MessageWithContent, SSEChatStreamMcpElicitationRequiredData } from '@/api-client/types'
import { ToolCallPendingApprovalContent } from '@/modules/mcp/chat-extension/components/ToolCallPendingApprovalContent'
import { McpMenuItem } from '@/modules/mcp/chat-extension/components/McpMenuItem'
import { McpConfigModal } from '@/modules/mcp/components/McpConfigModal'
import { McpStatusRow } from '@/modules/mcp/chat-extension/components/McpStatusRow'
import { McpInitializer } from '@/modules/mcp/chat-extension/components/McpInitializer'
import { ElicitationFormContent } from '@/modules/mcp/chat-extension/components/ElicitationFormContent'
import { JsToolApprovalContent } from '@/modules/mcp/chat-extension/components/JsToolApprovalContent'

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

  const serverLabel = mcpServerParenLabel(toolCall.server)

  return (
    <Card
      size="sm"
      className={cn('mb-2', !isExpanded && 'py-2.5')}
      data-testid={`mcp-toolcall-card-${toolCall.tool_use_id}`}
    >
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <ToolStatusIcon status={toolCall.status} />
          <Text strong className="truncate">{toolCall.tool_name}</Text>
          {serverLabel && (
            <Text type="secondary" className="text-xs whitespace-nowrap">
              {serverLabel}
            </Text>
          )}
          {/* Status is conveyed by the icon (check / x / wrench) — no text. A
              hidden marker keeps the completed/failed signal available to tests
              (mirrors the historical McpToolUseRenderer marker). */}
          {(toolCall.status === 'completed' || toolCall.status === 'error') && (
            <span
              className="sr-only"
              data-testid={`mcp-toolcall-status-${toolCall.tool_use_id}`}
              data-status={toolCall.status === 'error' ? 'failed' : 'completed'}
            />
          )}
        </div>
        <Button
          size="icon"
          variant="ghost"
          tooltip={isExpanded ? 'Hide details' : 'Show details'}
          icon={<ChevronDown className={cn('transition-transform', isExpanded && 'rotate-180')} />}
          onClick={() => setIsExpanded(!isExpanded)}
          data-testid={`mcp-toolcall-details-btn-${toolCall.tool_use_id}`}
        />
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
  // Hoisted above the early returns below: `Stores.Chat.messages` is a reactive
  // store-proxy access that calls a hook on every render, so it MUST run on every
  // render path — otherwise a re-render that early-returns (e.g. once `toolCall`
  // is tracked) calls fewer hooks → "Rendered fewer hooks than expected" crash.
  const { messages } = Stores.Chat
  const toolUseData = data.content as MessageContentDataToolUse

  if (!toolUseData.id) {
    return null
  }

  const toolCall = toolCalls.get(toolUseData.id)

  // If we have a tracked tool call, render it
  if (toolCall) {
    return <McpToolCallUI toolCall={toolCall} />
  }

  // Look up the server row so we can show its human display name (never the id).
  const server = servers.find(s => s.id === toolUseData.server_id)

  // Look up matching tool_result for historical display
  const message = messages.get(data.message_id)
  const toolResultData = message?.contents.find(
    c =>
      c.content_type === 'tool_result' &&
      ((c.content as unknown as { tool_use_id: string }).tool_use_id === toolUseData.id),
  )?.content as unknown as { content: string; is_error?: boolean } | undefined

  const hasDetails = toolUseData.input || toolResultData

  // Historical view for tool calls loaded from DB (store is empty after reload)
  return (
    <Card size="sm" className={cn('mb-2', !isExpanded && 'py-2.5')} data-testid={`mcp-tooluse-card-${toolUseData.id}`}>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2 min-w-0">
          <ToolStatusIcon
            status={toolResultData ? (toolResultData.is_error ? 'failed' : 'success') : 'running'}
          />
          <Text strong className="truncate">{toolUseData.name || 'Tool Call'}</Text>
          {mcpServerParenLabel(server?.display_name) && (
            <Text type="secondary" className="text-xs whitespace-nowrap">{mcpServerParenLabel(server?.display_name)}</Text>
          )}
          {/* Status is conveyed by the icon (check / x / wrench) — no text. A
              hidden marker keeps the completed/failed signal available to tests. */}
          {toolResultData && (
            <span
              className="sr-only"
              data-testid={`mcp-tooluse-status-${toolUseData.id}`}
              data-status={toolResultData.is_error ? 'failed' : 'completed'}
            />
          )}
        </div>
        {hasDetails && (
          <Button
            size="icon"
            variant="ghost"
            tooltip={isExpanded ? 'Hide details' : 'Show details'}
            icon={<ChevronDown className={cn('transition-transform', isExpanded && 'rotate-180')} />}
            onClick={() => setIsExpanded(!isExpanded)}
            data-testid={`mcp-tooluse-details-btn-${toolUseData.id}`}
          />
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
 * A maximal run of consecutive tool blocks starting at `index` — every
 * following `tool_use` / `tool_result` block, stopping at the first block of any
 * other type (e.g. an assistant `text`). Tool calls are emitted as adjacent
 * tool_use→tool_result pairs, so a run of K tool calls is up to 2K blocks.
 */
function collectToolRun(
  blocks: MessageContent[],
  index: number,
): MessageContent[] {
  const run: MessageContent[] = []
  for (let i = index; i < blocks.length; i++) {
    const t = blocks[i].content_type
    if (t !== 'tool_use' && t !== 'tool_result') break
    run.push(blocks[i])
  }
  return run
}

/** Number of tool_use blocks in a run (the "N tools" count). */
function countToolUses(run: MessageContent[]): number {
  return run.filter(b => b.content_type === 'tool_use').length
}

/**
 * Collapsed "N tools called" card for a run of ≥2 consecutive tool calls.
 * Expanding renders each run member through the registry's single-block path
 * (tool_use → its own card, tool_result → its files), so nothing regroups.
 */
function McpToolGroupCard({
  run,
  isUser,
}: {
  run: MessageContent[]
  isUser: boolean
}) {
  const [isExpanded, setIsExpanded] = useState(false)
  const toolUses = run.filter(b => b.content_type === 'tool_use')
  const resultByUseId = new Map<string, { is_error?: boolean }>()
  for (const b of run) {
    if (b.content_type !== 'tool_result') continue
    const rc = b.content as unknown as { tool_use_id?: string; is_error?: boolean }
    if (rc.tool_use_id) resultByUseId.set(rc.tool_use_id, rc)
  }
  const hasError = [...resultByUseId.values()].some(r => r.is_error)
  const allDone = toolUses.every(u =>
    resultByUseId.has((u.content as MessageContentDataToolUse).id),
  )
  const icon = (
    <ToolStatusIcon status={hasError ? 'failed' : !allDone ? 'running' : 'success'} />
  )

  return (
    <Card size="sm" className={cn('mb-2', !isExpanded && 'py-2.5')} data-testid="mcp-toolgroup-card">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          {icon}
          <Text strong>{toolUses.length} tools called</Text>
        </div>
        <Button
          size="icon"
          variant="ghost"
          tooltip={isExpanded ? 'Hide details' : 'Show details'}
          icon={<ChevronDown className={cn('transition-transform', isExpanded && 'rotate-180')} />}
          onClick={() => setIsExpanded(!isExpanded)}
          data-testid="mcp-toolgroup-details-btn"
        />
      </div>
      {isExpanded && (
        <div className="mt-2 flex flex-col gap-2">
          {run.map((b, i) => {
            // Single-block render (no `blocks`), so the group renderer falls back
            // to its single form — no recursion.
            const res = chatExtensionRegistry.renderContent({ content: b, isUser })
            return <Fragment key={b.id || `run-${i}`}>{res?.node ?? null}</Fragment>
          })}
        </div>
      )}
    </Card>
  )
}

/**
 * tool_use renderer entry point. Given its neighbor blocks (from ChatMessage's
 * run-loop), it folds a run of ≥2 consecutive tool calls into one
 * `McpToolGroupCard`; otherwise (a lone call, or rendered standalone without
 * neighbors) it defers to the single-tool `McpToolUseRenderer`. Its
 * `contentSpan` tells the run-loop how many blocks the group consumed.
 */
function McpToolUseGroup(props: ContentRendererProps) {
  const { content, isUser, blocks, index } = props
  const run =
    blocks && index != null ? collectToolRun(blocks, index) : null
  if (!run || countToolUses(run) < 2) {
    return <McpToolUseRenderer content={content} isUser={isUser} />
  }
  return <McpToolGroupCard run={run} isUser={isUser} />
}
McpToolUseGroup.contentSpan = (blocks: MessageContent[], index: number): number => {
  const run = collectToolRun(blocks, index)
  // Only swallow the whole run (use+result blocks) when it's an actual group;
  // a lone tool call consumes just its own block so its result renders normally.
  return countToolUses(run) >= 2 ? run.length : 1
}

/**
 * MCP Extension
 * Handles MCP tool calls, approval workflows, and renders tool call UI
 */
// Per-pane subscription teardown (ITEM-34/5), keyed by ctx.chatStore.
const paneMcpSubs = new WeakMap<object, Array<() => void>>()

const mcpExtension: ChatExtension = createExtension({
  name: 'mcp',
  description: 'Handles MCP tool calls and approval workflows',
  priority: 50, // Higher priority to handle events early

  initialize: async (ctx) => {
    const { Stores } = await import('@/core/stores')
    const { ApiClient } = await import('@/api-client')

    // Bind the editing-message restore to the OWNING pane's chat store
    // (ctx.chatStore, ITEM-34/5) so editing in a non-focused pane restores that
    // pane's MCP server selection. Unsub stored per-pane for cleanup.
    const chatStore = ctx.chatStore
    const subs: Array<() => void> = []
    paneMcpSubs.set(chatStore, subs)
    subs.push(
      chatStore.subscribe(
        (state: any) => state.editingMessage,
        async (editingMessage: any) => {
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
          } catch {
            // Soft-fail: no snapshot recorded (pre-migration message
            // or write hook failed at send-time) → keep current
            // selection. Matches the pre-extraction behavior for
            // messages without the column populated.
          }
        } else {
          // Edit cancelled or sent — restore from stored conversation config
          const conversation = chatStore.getState().conversation
          if (conversation) {
            mcpStore.setCurrentConversation(conversation.id)
          }
        }
        },
      ),
    )
  },

  // Type-safe SSE event handlers
  sseEventHandlers: {
    mcpToolStart: async (data, get, set) => {
      // data is automatically typed as SSEChatStreamMcpToolStartData
      // addToolCall is an action — callable directly on the store proxy
      // (actions are hook-free, safe outside a React component context).
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
        }
      }
    },

    runJsApprovalRequired: async (data, get, set) => {
      // A run_js script is SUSPENDED awaiting approval of a gated sub-tool call.
      // Inject a `run_js_approval` content block so JsToolApprovalContent renders
      // approve/deny; resolution goes via the side-channel elicitation /respond
      // endpoint (the same in-process oneshot ask_user uses), NOT the
      // turn-boundary tool_approvals flow — a live script stack can't survive a
      // request boundary.
      //
      // Register an elicitationRequests entry keyed by elicitation_id so
      // resolveElicitation can reflect the resolved status there (and roll it
      // back to 'pending' on a failed POST) — the component reads its status
      // from the store, so a failed approve re-enables the buttons and the
      // resolved state survives a component remount. Guard on !has() so a
      // double-delivered SSE frame can't reset an already-resolved entry to
      // 'pending' (which would re-show the buttons + allow a duplicate POST).
      if (!Stores.McpComposer.$.elicitationRequests.has(data.elicitation_id)) {
        Stores.McpComposer.addElicitationRequest({
          elicitation_id: data.elicitation_id,
          message: `run_js wants to call ${data.tool_name}`,
          requested_schema: {},
          server: data.server,
          message_id: null,
        } as unknown as SSEChatStreamMcpElicitationRequiredData)
      }

      const chatState = get()
      const streamingMessage = chatState.streamingMessage
      const now = new Date().toISOString()

      const approvalContent = {
        id: '',
        message_id: '',
        content_type: 'run_js_approval',
        content: {
          type: 'run_js_approval',
          status: 'pending',
          elicitation_id: data.elicitation_id,
          tool_name: data.tool_name,
          server: data.server,
          input: data.input,
        },
        sequence_order: 0,
        created_at: now,
        updated_at: now,
      } as unknown as MessageContent

      if (streamingMessage) {
        // Dedup by the unique per-approval elicitation_id.
        const exists = streamingMessage.contents.some(
          c =>
            c.content_type === 'run_js_approval' &&
            (c.content as unknown as { elicitation_id: string }).elicitation_id === data.elicitation_id,
        )
        if (!exists) {
          approvalContent.id = `${streamingMessage.id}-runjs-${data.elicitation_id}`
          approvalContent.message_id = streamingMessage.id
          approvalContent.sequence_order = streamingMessage.contents.length

          const updatedMessage = {
            ...streamingMessage,
            contents: [...streamingMessage.contents, approvalContent],
          }
          const newMessages = new Map(chatState.messages)
          newMessages.set(updatedMessage.id, updatedMessage)
          set({ streamingMessage: updatedMessage, messages: newMessages })
        }
      } else {
        // No streaming message (e.g. after reload / SSE-resume, since generation
        // is a detached server task) — create one so the approval prompt renders
        // and the suspended script can still be resumed. Mirrors
        // mcpElicitationRequired's else-branch.
        const messageId = `streaming-${Date.now()}`
        approvalContent.id = `${messageId}-runjs-${data.elicitation_id}`
        approvalContent.message_id = messageId

        const newMessage: MessageWithContent = {
          id: messageId,
          role: 'assistant',
          contents: [approvalContent],
          originated_from_id: '',
          edit_count: 0,
          created_at: now,
        }
        const newMessages = new Map(chatState.messages)
        newMessages.set(newMessage.id, newMessage)
        set({ streamingMessage: newMessage, messages: newMessages })
      }
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
    },

    mcpToolComplete: async (data, _get, _set) => {
      // data is automatically typed as SSEChatStreamMcpToolCompleteData
      const mcpStore = Stores.McpComposer

      mcpStore.updateToolCall(data.tool_use_id, {
        status: data.is_error ? 'error' : 'completed',
        error: data.is_error ? 'Tool execution failed' : undefined,
        result: data.result,
      })
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

    const fields: {
      enable_mcp?: boolean
      mcp_config?: { mcp_servers: typeof selectedServers }
      tool_approvals?: typeof approvalDecisions
    } = {}

    // Add MCP config if servers are selected
    if (selectedServers.length > 0) {
      fields.enable_mcp = true
      fields.mcp_config = { mcp_servers: selectedServers }
    }

    // Add approval decisions if present
    if (approvalDecisions.length > 0) {
      fields.tool_approvals = approvalDecisions
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
      // Read via `$` snapshot on the McpServer store (outside React context)
      const mcpServerState = Stores.McpServer.$
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
      }
    } catch {
      // If settings don't exist yet, create default config with all servers enabled
      const mcpServerState = Stores.McpServer.$
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
        }
      } catch (error) {
        console.error('[MCP Extension] Failed to load pending approvals:', error)
      }
    }
  },

  // Clear approval decisions after message is sent
  onMessageSent: async () => {
    const { Stores } = await import('@/core/stores')
    // Read via `$` snapshot (state fields + actions both live on getState())
    const mcpStore = Stores.McpComposer.$
    const chatStore = Stores.Chat.$

    // Get current conversation from chat store
    const conversation = chatStore.conversation

    // Handle new conversation creation
    if (conversation?.id && !mcpStore.currentConversationId) {
      // Transfer pending config to the new conversation
      mcpStore.transferPendingConfig(conversation.id)

      // Set current conversation ID
      mcpStore.setCurrentConversation(conversation.id)

      // Get available server IDs for proper disabled_servers computation
      const mcpServerState = Stores.McpServer.$
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
    tool_use: McpToolUseGroup,
    elicitation_request: ElicitationFormContent,
    run_js_approval: JsToolApprovalContent,
  },

  // Register slot components
  slots: {
    toolbar_actions: { component: McpInitializer, order: 1 },
    toolbar_plus_items: { component: McpMenuItem, order: 20 },
    toolbar_status: { component: McpStatusRow, order: 10 },
    // The config modal is hosted from an always-mounted composer slot (NOT the
    // "+" dropdown item) so it survives the dropdown closing on click.
    input_area_suffix: { component: McpConfigModal, order: 20 },
  },

  cleanup: async (ctx) => {
    const subs = paneMcpSubs.get(ctx.chatStore)
    if (subs) {
      for (const unsub of subs) unsub()
      paneMcpSubs.delete(ctx.chatStore)
    }
  },
})

export default mcpExtension

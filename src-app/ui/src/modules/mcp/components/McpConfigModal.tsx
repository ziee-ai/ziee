import { useEffect, useMemo, useState } from 'react'
import { Dialog, Accordion, Switch, Tag, Text, Title, Empty, Checkbox, Select, Separator, Button, InputNumber, message } from '@ziee/kit'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { Trash2 } from 'lucide-react'
import { Stores } from '@/core/stores'
import type { Tool } from '@/api-client/types'
import { PENDING_CONVERSATION_KEY, projectConfigKey } from '@/modules/mcp/stores/McpComposer.store'

/**
 * MCP Configuration Modal
 *
 * Self-contained modal for managing MCP settings for the current conversation:
 * - Server and tool selection
 * - Approval mode (disabled/auto_approve/manual_approve)
 * - Auto-approved tools list
 *
 * Features:
 * - Per-conversation configuration (persisted to backend)
 * - Works for both new and existing conversations
 * - Groups tools by MCP server
 * - Server-level toggle (enable all tools from server)
 * - Individual tool toggles
 * - Visibility controlled via store (Stores.McpComposer.openConfigModal/closeConfigModal)
 */
// The modal is a global-store-driven singleton, but it is mounted from more
// than one host (the chat composer's `input_area_suffix` slot AND the project
// MCP settings panel). On a page that renders both — e.g. the project detail
// page, which has an inline ChatInput *and* the project MCP panel — two
// instances would both open on the same global flag, duplicating the dialog.
// A module-level mount guard ensures only the first-mounted instance renders.
let mcpConfigModalMounts = 0

export function McpConfigModal() {
  // Below the mobile (sm) breakpoint — the same width at which the sidebar
  // becomes an overlay Sheet — the config surface slides in as a Drawer instead
  // of a centered Dialog (which is cramped once the viewport can't fit it).
  const isMobile = useWindowMinSize().sm
  const [isPrimaryModal, setIsPrimaryModal] = useState(false)
  useEffect(() => {
    mcpConfigModalMounts += 1
    if (mcpConfigModalMounts === 1) setIsPrimaryModal(true)
    return () => {
      mcpConfigModalMounts -= 1
    }
  }, [])

  const { servers } = Stores.McpServer  // Reactive access to MCP module store
  const mcpStore = Stores.McpComposer
  // Extract all store properties unconditionally at the top (store proxy uses hooks).
  // currentProjectId scopes the modal to a project's MCP defaults when set
  // alongside a null currentConversationId; both falsy = chat scope.
  const {
    selectedServers,
    currentConversationId,
    currentProjectId,
    conversationConfigs,
    configModalVisible,
  } = mcpStore

  // Project scope dispatch: project is in effect only when there is no
  // conversation context. A conversation that happens to belong to a
  // project still edits conversation overrides via the chat path.
  const isProjectScope = currentProjectId !== null && currentConversationId === null

  // Local state for tools (loaded on demand)
  const [serverTools, setServerTools] = useState<Map<string, Tool[]>>(new Map())
  const [loadingTools, setLoadingTools] = useState<Set<string>>(new Set())
  const [saving, setSaving] = useState(false)
  const [savingDefaults, setSavingDefaults] = useState(false)

  // Get enabled servers (available for selection). Memoized so the array
  // reference is stable across renders (it feeds effect deps / child props).
  const enabledServers = useMemo(() => servers.filter(s => s.enabled), [servers])

  // Get the current config keyed by scope. Project scope uses the
  // `project:<id>` namespaced key (set by openConfigModalForProject);
  // chat scope falls back to the conversation id (or the pending key
  // for new chats).
  const configKey = isProjectScope
    ? projectConfigKey(currentProjectId!)
    : currentConversationId || PENDING_CONVERSATION_KEY
  const conversationConfig = conversationConfigs.get(configKey)
  const approvalMode = conversationConfig?.approvalMode || 'manual_approve'
  const loopSettings = conversationConfig?.loopSettings || {
    stop_when_no_tool_calling: true,
    max_iteration: 10,
    stop_when_tools_called: [],
    force_final_answer: false,
    per_tool_max_iteration: [],
  }

  // Local state for select values
  const [stopToolValue, setStopToolValue] = useState<string | undefined>(undefined)
  const [perToolLimitValue, setPerToolLimitValue] = useState<string | undefined>(undefined)

  // Build grouped tool options for "Stop When Tools Called" picker (exclude already selected)
  const stopToolOptions = enabledServers.map(server => ({
    label: server.display_name,  // Group header
    options: (serverTools.get(server.id) || [])
      .filter(tool => {
        // Exclude tools already in stop_when_tools_called
        const alreadySelected = (loopSettings.stop_when_tools_called || []).some(
          t => t.server_id === server.id && t.tool_name === tool.name
        )
        return !alreadySelected
      })
      .map(tool => ({
        value: `${server.id}:${tool.name}`,
        label: tool.name,  // Just tool name (server shown in group header)
      })),
  })).filter(group => group.options.length > 0)  // Remove empty groups

  // Build grouped tool options for "Per-Tool Limits" picker (exclude already selected)
  const perToolLimitOptions = enabledServers.map(server => ({
    label: server.display_name,  // Group header
    options: (serverTools.get(server.id) || [])
      .filter(tool => {
        // Exclude tools already in per_tool_max_iteration
        const alreadySelected = (loopSettings.per_tool_max_iteration || []).some(
          t => t.server_id === server.id && t.tool_name === tool.name
        )
        return !alreadySelected
      })
      .map(tool => ({
        value: `${server.id}:${tool.name}`,
        label: tool.name,  // Just tool name (server shown in group header)
      })),
  })).filter(group => group.options.length > 0)  // Remove empty groups

  // Project scope: seed `selectedServers` from
  // `enabledServers - disabled_servers` so the modal shows the
  // server switches in the right state on open. Chat scope already
  // populates this via setCurrentConversation; the project path
  // skips that machinery, so do it once here when the modal opens.
  useEffect(() => {
    if (!configModalVisible || !isProjectScope) return
    // Only seed when the map is empty — preserves user toggles made
    // since open.
    if (selectedServers.size > 0) return
    const disabledIds = new Set(
      (conversationConfig?.disabledServers || [])
        .filter(d => (d.tools || []).length === 0)
        .map(d => d.server_id),
    )
    for (const server of enabledServers) {
      if (!disabledIds.has(server.id)) {
        // Default to "all tools selected" (empty array). Partial
        // disable from disabled_servers with non-empty tools will be
        // honored on save via the inversion in saveProjectConfig.
        mcpStore.selectServer(server.id, [])
      }
    }
    // We intentionally exclude `selectedServers` + `conversationConfig`
    // from deps — they change on every selectServer call and would
    // re-trigger the seed loop. The size-guard above is the
    // run-once gate.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [configModalVisible, isProjectScope, enabledServers.length])

  // Lazy load tools when modal opens
  useEffect(() => {
    if (configModalVisible) {
      enabledServers.forEach(async server => {
        if (!serverTools.has(server.id) && !loadingTools.has(server.id)) {
          setLoadingTools(prev => new Set(prev).add(server.id))

          try {
            const response = await Stores.McpComposer.listServerTools(server.id)
            setServerTools(prev => new Map(prev).set(server.id, response.tools))
          } catch (error) {
            console.error('[MCP Config Modal] Failed to load tools for server:', server.id, error)
          } finally {
            setLoadingTools(prev => {
              const next = new Set(prev)
              next.delete(server.id)
              return next
            })
          }
        }
      })
    }
  }, [configModalVisible, enabledServers.length])

  // Check if a specific tool is selected
  const isToolSelected = (serverId: string, toolName: string): boolean => {
    const selection = selectedServers.get(serverId)
    if (!selection) return false
    // If tools array is empty, all tools are selected
    if (selection.tools.length === 0) return true
    // Otherwise check if tool is in the array
    return selection.tools.includes(toolName)
  }

  // Handle server-level toggle
  const handleServerToggle = (serverId: string, checked: boolean) => {
    if (checked) {
      // Select server with all tools (empty array)
      mcpStore.selectServer(serverId, [])
    } else {
      // Deselect server
      mcpStore.deselectServer(serverId)
    }
  }

  // Handle individual tool toggle
  const handleToolToggle = (serverId: string, toolName: string) => {
    const selection = selectedServers.get(serverId)

    if (!selection) {
      // Server not selected, select it with just this tool
      mcpStore.selectServer(serverId, [toolName])
      return
    }

    if (selection.tools.length === 0) {
      // All tools selected, switch to explicit selection
      const tools = serverTools.get(serverId) || []
      const otherTools = tools
        .filter(t => t.name !== toolName)
        .map(t => t.name)
      mcpStore.selectServer(serverId, otherTools)
    } else {
      // Some tools selected, toggle this one
      mcpStore.toggleServerTool(serverId, toolName)
    }
  }

  // Handle approval mode change (works for both existing and new conversations)
  const handleApprovalModeChange = (value: string) => {
    mcpStore.setApprovalMode(currentConversationId, value as 'disabled' | 'auto_approve' | 'manual_approve')
  }

  // Handle save. Dispatches by scope:
  //   - Project scope (currentProjectId set + currentConversationId null):
  //     PUT /projects/{id}/mcp-settings via saveProjectConfig.
  //   - Existing conversation: existing saveConversationConfig path.
  //   - Pending conversation (both null): kept in the pending buffer,
  //     persisted on first message when the conversation is created.
  const handleSave = async () => {
    if (!isProjectScope && !currentConversationId) {
      // Settings stay in pending config and persist on first message.
      return
    }

    setSaving(true)
    try {
      const availableServerIds = enabledServers.map(s => s.id)
      const serverToolsMap = new Map(
        Array.from(serverTools.entries()).map(([id, tools]) => [id, tools.map(t => t.name)])
      )
      if (isProjectScope) {
        await mcpStore.saveProjectConfig(currentProjectId!, availableServerIds, serverToolsMap)
      } else {
        await mcpStore.saveConversationConfig(currentConversationId!, availableServerIds, serverToolsMap)
      }
    } catch (error) {
      console.error('[MCP Config Modal] Failed to save configuration:', error)
      message.error(
        error instanceof Error ? error.message : 'Failed to save MCP configuration',
      )
    } finally {
      setSaving(false)
    }
  }

  // Auto-save on close in any scope that persists immediately
  // (existing conversation OR project). Pending-conversation scope
  // skips the network call — its config buffers until first message.
  const handleClose = async () => {
    if (currentConversationId || isProjectScope) {
      await handleSave()
    }
    mcpStore.closeConfigModal()
  }

  // Handle save as default
  const handleSaveAsDefault = async () => {
    setSavingDefaults(true)
    try {
      const availableServerIds = enabledServers.map(s => s.id)
      await mcpStore.saveUserDefaults(currentConversationId, availableServerIds, true)
    } catch (error) {
      console.error('[MCP Config Modal] Failed to save as defaults:', error)
    } finally {
      setSavingDefaults(false)
    }
  }

  // Check if a tool is auto-approved
  const isToolAutoApproved = (serverId: string, toolName: string): boolean => {
    const autoApprovedTools = conversationConfig?.autoApprovedTools || []
    const serverEntry = autoApprovedTools.find(s => s.server_id === serverId)
    return serverEntry ? serverEntry.tools.includes(toolName) : false
  }

  // Handle auto-approve toggle (works for both existing and new conversations)
  const handleAutoApproveToggle = (serverId: string, toolName: string) => {
    mcpStore.toggleAutoApprovedTool(currentConversationId, serverId, toolName)
  }

  // Build collapse items
  const collapseItems = enabledServers.map(server => {
    const tools = serverTools.get(server.id) || []
    const selection = selectedServers.get(server.id)

    return {
      key: server.id,
      label: (
        <div className="flex items-center justify-between w-full" data-testid={`mcp-config-server-row-${server.id}`}>
          <div className="flex items-center gap-2">
            {/* Stop the toggle's own click from bubbling to the accordion trigger,
                so flipping the server switch doesn't also expand/collapse the row. */}
            <span onClick={(e) => e.stopPropagation()} className="inline-flex">
              <Switch
                tooltip="Enable this server for the conversation"
                checked={!!selection}
                onChange={(checked) => handleServerToggle(server.id, checked)}
                size="sm"
                data-testid={`mcp-config-server-switch-${server.id}`}
              />
            </span>
            <Text strong>{server.display_name}</Text>
            <Tag variant="outline" tone={server.user_id ? 'info' : 'success'} className="text-xs" data-testid={`mcp-config-server-tag-${server.id}`}>
              {server.user_id ? 'User' : 'System'}
            </Tag>
          </div>
          <Text type="secondary" className="text-xs">
            {tools.length} tool{tools.length !== 1 ? 's' : ''}
          </Text>
        </div>
      ),
      children: tools.length === 0 ? (
        <Empty description="No tools yet" data-testid={`mcp-config-server-empty-${server.id}`} />
      ) : (
        <div className="space-y-2">
          {tools.map(tool => (
            <div key={tool.name} className="flex items-start gap-2 p-2 rounded">
              <Checkbox
                checked={isToolSelected(server.id, tool.name)}
                onChange={() => handleToolToggle(server.id, tool.name)}
                disabled={!selection}
                data-testid={`mcp-config-tool-checkbox-${server.id}-${tool.name}`}
              />
              <div className="flex-1">
                <div className="flex items-center justify-between">
                  <Text strong className="text-sm">{tool.name}</Text>
                  {approvalMode === 'manual_approve' && (
                    <div className="flex items-center gap-1">
                      <Text type="secondary" className="text-xs">Auto Approve</Text>
                      <Switch
                        tooltip="Auto-approve this tool"
                        size="sm"
                        checked={isToolAutoApproved(server.id, tool.name)}
                        onChange={() => handleAutoApproveToggle(server.id, tool.name)}
                        disabled={!selection}
                        data-testid={`mcp-config-tool-approve-${server.id}-${tool.name}`}
                      />
                    </div>
                  )}
                </div>
                {tool.description && (
                  <div className="text-xs text-muted-foreground mt-1">{tool.description}</div>
                )}
              </div>
            </div>
          ))}
        </div>
      ),
    }
  })

  // Only the first-mounted instance renders the (singleton) dialog; any extra
  // mount points no-op so the same global open-flag can't show two dialogs.
  if (!isPrimaryModal) return null

  const modalTitle = isProjectScope ? 'MCP Defaults for Project' : 'MCP Configuration'
  const modalFooter = (
    // Right-aligned regardless of host (the Drawer only auto-right-aligns ARRAY
    // footers, not a node) — actions belong on the trailing edge in both.
    <div className="flex justify-end gap-2">
      {/* "Save as Default" writes user_mcp_defaults — orthogonal to
          project scope, hide there to avoid confusion. */}
      {!isProjectScope && (
        <Button onClick={handleSaveAsDefault} loading={savingDefaults} data-testid="mcp-config-save-default-btn">
          Save as Default
        </Button>
      )}
      <Button onClick={handleClose} loading={saving} data-testid="mcp-config-close-btn">
        {isProjectScope || currentConversationId ? 'Save & Close' : 'Close'}
      </Button>
    </div>
  )
  const modalBody = (
    <div className="space-y-4 pb-2">
        {/* Approval Mode Section */}
        <div>
          <Title level={5} className="!text-sm">Approval Mode</Title>
          <Select
            value={approvalMode}
            onChange={handleApprovalModeChange}
            className="w-full mt-2"
            data-testid="mcp-config-approval-select"
            options={[
              {
                value: 'disabled',
                label: 'Disabled - MCP tools blocked',
              },
              {
                value: 'auto_approve',
                label: 'Auto Approve - Automatically approve all tools',
              },
              {
                value: 'manual_approve',
                label: 'Manual Approve - Require approval for each tool',
              },
            ]}
          />
        </div>

        <Separator />

        {/* Server & Tool Selection Section */}
        <div>
          <Title level={5} className="!text-sm">Server & Tool Selection</Title>
          {enabledServers.length === 0 ? (
            <Empty description="No MCP servers available" data-testid="mcp-config-empty-servers" />
          ) : (
            <Accordion collapsible items={collapseItems} data-testid="mcp-config-servers-accordion" />
          )}
        </div>

        <Separator />

        {/* Loop Settings Section */}
        <div>
          <Title level={5} className="!text-sm">Loop Settings</Title>

          {/* Boolean toggles */}
          <div className="space-y-2 mb-4 mt-2">
            <div className="flex items-center justify-between">
              <Text>Stop when AI doesn't call any tools</Text>
              <Switch
                tooltip="Stop when the AI calls no tools"
                checked={loopSettings.stop_when_no_tool_calling ?? true}
                onChange={(checked) => mcpStore.setLoopSettings(currentConversationId, { stop_when_no_tool_calling: checked })}
                data-testid="mcp-config-stop-no-tools-switch"
              />
            </div>
            <div className="flex items-center justify-between">
              <Text>Force final answer when limits reached</Text>
              <Switch
                tooltip="Force a final answer when limits are reached"
                checked={loopSettings.force_final_answer ?? false}
                onChange={(checked) => mcpStore.setLoopSettings(currentConversationId, { force_final_answer: checked })}
                data-testid="mcp-config-force-final-switch"
              />
            </div>
          </div>

          {/* Max iterations */}
          <div className="flex items-center gap-2 mb-4">
            <Text>Max Iterations:</Text>
            <InputNumber
              min={0}
              value={loopSettings.max_iteration ?? 10}
              onChange={(value) => mcpStore.setLoopSettings(currentConversationId, { max_iteration: value ?? 10 })}
              className="w-[100px]"
              aria-label="Max iterations"
              data-testid="mcp-config-max-iter-input"
            />
            <Text type="secondary" className="text-xs">(0 = unlimited)</Text>
          </div>

          {/* Stop when tools called */}
          <div className="mb-4">
            <Text strong>Stop When These Tools Are Called:</Text>
            <div className="mt-2 flex flex-wrap gap-1">
              {(loopSettings.stop_when_tools_called || []).map((tool) => (
                <Tag variant="outline"
                  key={`${tool.server_id}-${tool.tool_name}`}
                  onClose={() => mcpStore.removeStopWhenToolCalled(currentConversationId, tool.server_id, tool.tool_name)}
                  closeLabel="Remove"
                  data-testid={`mcp-config-stop-tag-${tool.server_id}-${tool.tool_name}`}
                >
                  {enabledServers.find(s => s.id === tool.server_id)?.display_name || tool.server_id}/{tool.tool_name}
                </Tag>
              ))}
            </div>
            <Select
              placeholder="Add stop tool..."
              className="w-full mt-2"
              data-testid="mcp-config-stop-tool-select"
              options={stopToolOptions}
              value={stopToolValue}
              onChange={(value) => {
                if (value) {
                  const [serverId, ...rest] = value.split(':')
                  const toolName = rest.join(':')
                  mcpStore.addStopWhenToolCalled(currentConversationId, { server_id: serverId, tool_name: toolName })
                  setStopToolValue(undefined)  // Reset to allow re-selecting same value
                }
              }}
            />
          </div>

          {/* Per-tool limits */}
          <div>
            <Text strong>Per-Tool Iteration Limits:</Text>
            <div className="mt-2 space-y-2">
              {(loopSettings.per_tool_max_iteration || []).map((limit) => (
                <div key={`${limit.server_id}-${limit.tool_name}`} className="flex items-center gap-2">
                  <Text className="flex-1">
                    {enabledServers.find(s => s.id === limit.server_id)?.display_name || limit.server_id}/{limit.tool_name}
                  </Text>
                  <InputNumber
                    min={1}
                    value={limit.max_iteration}
                    onChange={(value) => mcpStore.updatePerToolLimit(currentConversationId, limit.server_id, limit.tool_name, value ?? 1)}
                    className="w-[80px]"
                    data-testid={`mcp-config-pertool-input-${limit.server_id}-${limit.tool_name}`}
                  />
                  <Button
                    variant="outline"
                    icon={<Trash2 />}
                    onClick={() => mcpStore.removePerToolLimit(currentConversationId, limit.server_id, limit.tool_name)}
                    data-testid={`mcp-config-pertool-remove-${limit.server_id}-${limit.tool_name}`}
                  />
                </div>
              ))}
            </div>
            <Select
              placeholder="Add per-tool limit..."
              className="w-full mt-2"
              data-testid="mcp-config-pertool-select"
              options={perToolLimitOptions}
              value={perToolLimitValue}
              onChange={(value) => {
                if (value) {
                  const [serverId, ...rest] = value.split(':')
                  const toolName = rest.join(':')
                  mcpStore.addPerToolLimit(currentConversationId, { server_id: serverId, tool_name: toolName, max_iteration: 3 })
                  setPerToolLimitValue(undefined)  // Reset to allow re-selecting same value
                }
              }}
            />
          </div>
        </div>
      </div>
  )

  return isMobile ? (
    <Drawer
      open={configModalVisible}
      onClose={handleClose}
      title={modalTitle}
      footer={modalFooter}
      placement="right"
      size="large"
      data-testid="mcp-config-modal"
    >
      {modalBody}
    </Drawer>
  ) : (
    <Dialog
      open={configModalVisible}
      onOpenChange={(v) => { if (!v) handleClose() }}
      className="max-w-[800px]"
      data-testid="mcp-config-modal"
      title={modalTitle}
      footer={modalFooter}
    >
      {modalBody}
    </Dialog>
  )
}

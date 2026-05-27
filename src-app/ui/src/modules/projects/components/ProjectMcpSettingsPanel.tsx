import { Button, Tag, Typography } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type Project } from '@/api-client/types'
import { McpConfigModal } from '@/modules/chat/extensions/mcp/components/McpConfigModal'

const { Text } = Typography

interface ProjectMcpSettingsPanelProps {
  project: Project
}

/**
 * Project MCP defaults editor. Round-4 redesign: instead of a
 * standalone raw-JSON form, this panel opens the SAME `McpConfigModal`
 * used in chat — driven by `Stores.Chat.McpStore.openConfigModalForProject`.
 *
 * The modal's dispatch rule is:
 *   currentProjectId != null && currentConversationId == null  →  project scope
 *
 * So opening from here saves to `PUT /projects/{id}/mcp-settings`
 * (via `saveProjectConfig`); opening from chat continues to save to
 * the conversation row. Settings are snapshotted onto every NEW
 * conversation created in this project — changes here do NOT
 * propagate to existing conversations.
 *
 * Permission gating (audit Q3): the configure button is hidden when
 * the user lacks `ProjectsEdit`; a read-only summary is shown
 * instead. Admins see both.
 */
export function ProjectMcpSettingsPanel({
  project,
}: ProjectMcpSettingsPanelProps) {
  const canEdit = usePermission(Permissions.ProjectsEdit)

  const handleConfigure = () => {
    Stores.Chat.McpStore.openConfigModalForProject(project)
  }

  const approvalMode = project.mcp_approval_mode || 'manual_approve'
  const approvalLabel =
    approvalMode === 'auto_approve'
      ? 'Auto approve'
      : approvalMode === 'disabled'
      ? 'Disabled'
      : 'Manual approve'

  const autoApprovedCount = Array.isArray(project.mcp_auto_approved_tools)
    ? (project.mcp_auto_approved_tools as unknown[]).length
    : 0
  const disabledCount = Array.isArray(project.mcp_disabled_servers)
    ? (project.mcp_disabled_servers as unknown[]).length
    : 0

  return (
    <div>
      <Text type="secondary" className="block mb-4">
        Default MCP approval mode and per-server settings for every NEW
        conversation in this project. Existing conversations keep their own
        settings — changes here do not retroactively apply.
      </Text>

      <div className="flex flex-col gap-3 mb-4">
        <div className="flex items-center gap-2">
          <Text strong>Approval mode:</Text>
          <Tag>{approvalLabel}</Tag>
        </div>
        <div className="flex items-center gap-2">
          <Text strong>Auto-approved server rules:</Text>
          <Tag color={autoApprovedCount > 0 ? 'blue' : 'default'}>
            {autoApprovedCount}
          </Tag>
        </div>
        <div className="flex items-center gap-2">
          <Text strong>Disabled server rules:</Text>
          <Tag color={disabledCount > 0 ? 'warning' : 'default'}>
            {disabledCount}
          </Tag>
        </div>
      </div>

      {canEdit && (
        <Button
          type="primary"
          icon={<ToolOutlined />}
          onClick={handleConfigure}
        >
          Configure MCP
        </Button>
      )}

      {/* Mount the shared modal here. It controls its own visibility
          via the chat MCP store. Mounting on the chat surface AND here
          would render two instances — but the modal is render-once
          per visible container in practice (this panel is only on the
          project detail page; the chat surface mounts via McpMenuItem,
          which renders null when the panel mounts a different one).
          To be safe both renderers gate on the same store flag, so
          the worst case is two empty <Modal> wrappers — harmless. */}
      <McpConfigModal />
    </div>
  )
}

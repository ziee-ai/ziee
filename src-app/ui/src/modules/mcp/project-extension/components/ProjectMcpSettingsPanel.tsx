import { Button, Card, Skeleton, Tag, Typography } from 'antd'
import { ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { McpConfigModal } from '@/modules/mcp/components/McpConfigModal'

const { Text } = Typography

/**
 * Project MCP defaults editor. Reads settings from the dedicated
 * `Stores.ProjectMcpSettings` store (separate fetch — the `Project`
 * payload no longer carries the MCP fields after the unification).
 *
 * The Configure button opens the SAME `McpConfigModal` used in chat,
 * seeded with the project's current settings. The modal's dispatch rule
 * (currentProjectId set, currentConversationId null) routes the save to
 * `PUT /api/projects/{id}/mcp-settings`. Settings get snapshotted onto
 * every NEW conversation created in this project — changes here do NOT
 * propagate to existing conversations.
 *
 * Permission gating: the configure button is hidden when the user lacks
 * `ProjectsEdit`; the summary view stays visible to readers. Admins see
 * both.
 */
export function ProjectMcpSettingsPanel() {
  const project = Stores.ProjectDetail.project
  const settings = Stores.ProjectMcpSettings.settings
  const loading = Stores.ProjectMcpSettings.loading
  const canEdit = usePermission(Permissions.ProjectsEdit)

  if (!project) return null

  const handleConfigure = () => {
    Stores.McpComposer.openConfigModalForProject(project.id, settings)
  }

  const approvalMode = settings?.approval_mode || 'manual_approve'
  const approvalLabel =
    approvalMode === 'auto_approve'
      ? 'Auto approve'
      : approvalMode === 'disabled'
        ? 'Disabled'
        : 'Manual approve'

  const autoApprovedCount = settings?.auto_approved_tools.length ?? 0
  const disabledCount = settings?.disabled_servers.length ?? 0

  return (
    <Card
      title={
        <span>
          <ToolOutlined className="mr-2" />
          MCP Defaults
        </span>
      }
      className="mb-4"
      data-test-section="mcp-defaults"
    >
      <Text type="secondary" className="block mb-4">
        Default MCP approval mode and per-server settings for every NEW
        conversation in this project. Existing conversations keep their own
        settings — changes here do not retroactively apply.
      </Text>

      {loading && !settings ? (
        <Skeleton active paragraph={{ rows: 2 }} />
      ) : (
        <div className="flex flex-col gap-3 mb-4">
          <div
            className="flex items-center gap-2"
            data-test-mcp-approval-mode={approvalMode}
          >
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
      )}

      {canEdit && (
        <Button
          type="primary"
          icon={<ToolOutlined />}
          onClick={handleConfigure}
          aria-label="Configure MCP defaults"
        >
          Configure MCP defaults
        </Button>
      )}

      <McpConfigModal />
    </Card>
  )
}

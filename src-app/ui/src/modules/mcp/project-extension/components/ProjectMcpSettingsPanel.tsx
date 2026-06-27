import { Button, Card, Empty, Skeleton, Space, Tag, Text } from '@/components/ui'
import { EditOutlined, ToolOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type AutoApprovedServer,
  type DisabledServer,
} from '@/api-client/types'
import { McpConfigModal } from '@/modules/mcp/components/McpConfigModal'

/**
 * Project MCP defaults editor. Reads settings from the dedicated
 * `Stores.ProjectMcpSettings` store (separate fetch — the `Project`
 * payload no longer carries the MCP fields after the unification).
 *
 * The Edit button (in the Card header) opens the SAME `McpConfigModal`
 * used in chat, seeded with the project's current settings. The modal's
 * dispatch rule (currentProjectId set, currentConversationId null)
 * routes the save to `PUT /api/projects/{id}/mcp-settings`. Settings
 * get snapshotted onto every NEW conversation created in this project
 * — changes here do NOT propagate to existing conversations.
 *
 * Permission gating: the Edit affordance is hidden when the user lacks
 * `ProjectsEdit`; the summary view stays visible to readers. Admins
 * see both.
 */
export function ProjectMcpSettingsPanel() {
  const project = Stores.ProjectDetail.project
  const settings = Stores.ProjectMcpSettings.settings
  const loading = Stores.ProjectMcpSettings.loading
  // Resolve server_id → display_name for the per-server lists below.
  // Falls back to the raw id when a server has been deleted but its
  // rule still references it.
  const { servers } = Stores.McpServer
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

  const serverName = (id: string) =>
    servers.find(s => s.id === id)?.display_name ?? id

  const renderServerRule = (
    rule: AutoApprovedServer | DisabledServer,
    tone: 'info' | 'warning',
  ) => {
    // Convention (see McpConfigModal.tsx:121): an empty `tools` array
    // means the rule applies to the whole server; a non-empty list
    // restricts the rule to those specific tool names.
    const allTools = rule.tools.length === 0
    return (
      <div key={rule.server_id} className="flex flex-col gap-1">
        <Text strong className="!text-sm">
          {serverName(rule.server_id)}
        </Text>
        {allTools ? (
          <Tag tone={tone}>All tools</Tag>
        ) : (
          <Space size={[4, 4]} wrap>
            {rule.tools.map(t => (
              <Tag key={t} tone={tone}>
                {t}
              </Tag>
            ))}
          </Space>
        )}
      </div>
    )
  }

  // The backend stores auto_approved_tools and disabled_servers
  // independently: the modal intentionally preserves your auto-approve
  // selections when you disable a server (so toggling it back on
  // restores your prior preferences — see
  // McpComposer.store.ts:475+). That means it's NORMAL for a fully-
  // disabled server to also have a stale auto-approve entry on disk.
  // It would just be confusing to render both as if they were both
  // active rules — auto-approve is meaningless while the server can't
  // be called. Filter the auto-approve list down by removing servers
  // that are fully disabled (entry in disabled_servers with no
  // per-tool restriction, i.e. tools.length === 0).
  const rawAutoApproved = settings?.auto_approved_tools ?? []
  const disabled = settings?.disabled_servers ?? []
  const fullyDisabledServerIds = new Set(
    disabled.filter(d => d.tools.length === 0).map(d => d.server_id),
  )
  const autoApproved = rawAutoApproved.filter(
    a => !fullyDisabledServerIds.has(a.server_id),
  )
  const noRules = autoApproved.length === 0 && disabled.length === 0

  return (
    <Card
      title={
        <span>
          <ToolOutlined className="mr-2" />
          MCP Defaults
        </span>
      }
      // Card header `extra` slot — moves the edit affordance out of the
      // body so it sits inline with the title, mirroring the other
      // project-detail cards (Knowledge, etc.).
      extra={
        canEdit && (
          <Button
            variant="ghost"
            icon={<EditOutlined />}
            onClick={handleConfigure}
            aria-label="Edit MCP defaults"
          >
            Edit
          </Button>
        )
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
        <Skeleton />
      ) : (
        <div className="flex flex-col gap-4">
          {/* Approval mode — always shown. */}
          <div
            className="flex items-center gap-2"
            data-test-mcp-approval-mode={approvalMode}
          >
            <Text strong>Approval mode:</Text>
            <Tag>{approvalLabel}</Tag>
          </div>

          {/* Auto-approved + disabled rule lists. Each section is
              hidden when empty unless BOTH are empty — in which case
              we surface a single neutral empty state below. */}
          {autoApproved.length > 0 && (
            <div className="flex flex-col gap-2">
              <Text strong>Auto-approved</Text>
              <div className="flex flex-col gap-3 pl-2">
                {autoApproved.map(r => renderServerRule(r, 'info'))}
              </div>
            </div>
          )}

          {disabled.length > 0 && (
            <div className="flex flex-col gap-2">
              <Text strong>Disabled</Text>
              <div className="flex flex-col gap-3 pl-2">
                {disabled.map(r => renderServerRule(r, 'warning'))}
              </div>
            </div>
          )}

          {noRules && (
            <Empty
              description={
                <Text type="secondary" className="!text-xs">
                  No per-server rules configured.
                </Text>
              }
            />
          )}
        </div>
      )}

      <McpConfigModal />
    </Card>
  )
}

import { useState } from 'react'
import {
  Button,
  Empty,
  List,
  Popconfirm,
  Spin,
  Tag,
  Tooltip,
  Typography,
  message,
} from 'antd'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Installed entities (assistants / MCP servers / models that were
 * created from a hub item) whose `hub_version` lags the current
 * catalog. Admin-only — the underlying endpoint
 * `GET /api/hub/updates` is gated on `hub::catalog::read`.
 */
export function UpdatesHubTab() {
  const updates = Stores.HubUpdates.updates
  const loading = Stores.HubUpdates.loading
  const error = Stores.HubUpdates.error
  const catalogVersion = Stores.HubUpdates.catalogVersion
  const navigate = useNavigate()
  const [busyId, setBusyId] = useState<string | null>(null)

  // Re-install an assistant / MCP server from the current catalog.
  // Both only need the hub_id (no provider/quant), so it's a one-click
  // op. Models need a provider + quantization choice, so those route
  // to the Models tab instead of installing inline.
  //
  // For assistants: branches on `is_template_install` — a
  // template-origin row (created_by IS NULL) must re-install via
  // `createAssistantTemplateFromHub`, otherwise the admin would
  // silently get a USER assistant owned by themselves and the stale
  // template would remain outdated.
  //
  // For MCP servers: same pattern — branches on
  // `is_system_mcp_install`. A system-origin row must re-install via
  // `createSystemMcpServerFromHub` (with `replace_existing: true`),
  // otherwise the admin would silently get a personal MCP server and
  // the stale system row would remain outdated.
  const reinstall = async (
    hubId: string,
    category: string,
    entityId: string,
    isTemplateInstall: boolean,
    isSystemMcpInstall: boolean,
  ) => {
    setBusyId(entityId)
    try {
      if (category === 'assistant') {
        if (isTemplateInstall) {
          // Route through the store so the displaced template's
          // `assistant_template.deleted` + the new template's
          // `assistant_template.created` events fire, keeping the
          // TemplateAssistants store + hub cards in sync.
          //
          // `replace_existing: true` instructs the template handler
          // to delete the outdated template first before creating
          // the fresh one — without this the duplicate-prevention
          // guard would 409 on re-install.
          await Stores.HubAssistants.createTemplateFromHub({
            hub_id: hubId,
            replace_existing: true,
          })
        } else {
          // User assistant: also pass `replace_existing: true` so the
          // backend deletes the user's prior install for this hub_id
          // before creating the new one. Without it, the Re-install
          // would create a duplicate row and `list_outdated_entities`
          // would keep surfacing the old stale-version row forever.
          await Stores.HubAssistants.createFromHub({
            hub_id: hubId,
            replace_existing: true,
          })
        }
      } else if (category === 'mcp_server') {
        if (isSystemMcpInstall) {
          // Route through the store so displaced + fresh server
          // events fire (mcp_server.deleted for the old uuid,
          // mcp_server.created for the new), keeping the
          // SystemMcpServers store + hub cards in sync. Without
          // the store seam, the admin MCP servers list would
          // keep the OLD (now-deleted) row.
          await Stores.HubMcpServers.createSystemFromHub({
            hub_id: hubId,
            replace_existing: true,
          })
        } else {
          // User MCP server: same Re-install semantics as user
          // assistants above — the backend deletes the user's prior
          // install before creating the new one.
          await Stores.HubMcpServers.createFromHub({
            hub_id: hubId,
            replace_existing: true,
          })
        }
      }
      message.success(`Re-installed ${hubId} from v${catalogVersion ?? '?'}`)
      await Stores.HubUpdates.loadUpdates()
    } catch (e) {
      message.error(`Failed to re-install ${hubId}: ${(e as Error)?.message ?? e}`)
    } finally {
      setBusyId(null)
    }
  }

  if (loading && updates.length === 0) {
    return (
      <div className="flex justify-center items-center py-12">
        <Spin />
      </div>
    )
  }

  if (error && updates.length === 0) {
    return (
      <Empty
        description={
          <Text type="secondary">
            Couldn't load updates: {error}
          </Text>
        }
      />
    )
  }

  if (updates.length === 0) {
    return (
      <Empty
        description={
          <Text type="secondary">
            Every installed hub item is on the current catalog
            {catalogVersion ? ` (v${catalogVersion})` : ''}.
          </Text>
        }
      />
    )
  }

  return (
    <div className="px-3">
      <Text type="secondary" className="block mb-3">
        {updates.length} installed item{updates.length === 1 ? '' : 's'} behind
        catalog{catalogVersion ? ` v${catalogVersion}` : ''}.
      </Text>
      <List
        bordered
        dataSource={updates}
        renderItem={row => (
          <List.Item
            key={`${row.entity_type}:${row.entity_id}`}
            actions={[
              <Tag key="installed" color="orange">
                installed{' '}
                {row.installed_version
                  ? `v${row.installed_version}`
                  : 'pre-tracking'}
              </Tag>,
              <Tag key="current" color="green">
                current v{row.current_version}
              </Tag>,
              row.is_template_install ? (
                <Tag key="scope" color="purple">
                  template
                </Tag>
              ) : row.is_system_mcp_install ? (
                <Tag key="scope" color="purple">
                  system
                </Tag>
              ) : null,
              row.hub_category === 'model' ? (
                <Tooltip
                  key="action"
                  title="Models update via the Models tab (pick a provider + quantization)"
                >
                  <Button
                    size="small"
                    type="link"
                    onClick={() => navigate('/hub/models')}
                  >
                    Update in Models
                  </Button>
                </Tooltip>
              ) : (
                <Popconfirm
                  key="action"
                  title="Re-install from current catalog"
                  description={
                    row.is_template_install
                      ? `Re-install template "${row.hub_id}" from catalog v${row.current_version}? The existing template will be replaced once the new one is created.`
                      : row.is_system_mcp_install
                        ? `Re-install system MCP server "${row.hub_id}" from catalog v${row.current_version}? The existing system server will be replaced once the new one is created.`
                        : `Create a fresh "${row.hub_id}" from catalog v${row.current_version}?`
                  }
                  okText="Re-install"
                  cancelText="Cancel"
                  onConfirm={() =>
                    reinstall(
                      row.hub_id,
                      row.hub_category,
                      row.entity_id,
                      row.is_template_install,
                      row.is_system_mcp_install,
                    )
                  }
                >
                  <Button
                    size="small"
                    type="link"
                    loading={busyId === row.entity_id}
                  >
                    Re-install
                  </Button>
                </Popconfirm>
              ),
            ]}
          >
            <List.Item.Meta
              title={<Text strong>{row.hub_id}</Text>}
              description={
                <Text type="secondary">
                  {row.hub_category} · {row.entity_type}
                </Text>
              }
            />
          </List.Item>
        )}
      />
    </div>
  )
}

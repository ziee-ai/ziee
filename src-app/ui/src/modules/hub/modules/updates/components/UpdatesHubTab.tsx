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
import { ApiClient } from '@/api-client'
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

  // Re-install an assistant / MCP server from the current catalog. Both
  // only need the hub_id (no provider/quant), so it's a one-click op.
  // Models need a provider + quantization choice, so those route to the
  // Models tab instead of installing inline.
  const reinstall = async (
    hubId: string,
    category: string,
    entityId: string,
  ) => {
    setBusyId(entityId)
    try {
      if (category === 'assistant') {
        await ApiClient.Hub.createAssistantFromHub({ hub_id: hubId })
      } else if (category === 'mcp_server') {
        await ApiClient.Hub.createMcpServerFromHub({ hub_id: hubId })
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
                  description={`Create a fresh "${row.hub_id}" from catalog v${row.current_version}?`}
                  okText="Re-install"
                  cancelText="Cancel"
                  onConfirm={() =>
                    reinstall(row.hub_id, row.hub_category, row.entity_id)
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

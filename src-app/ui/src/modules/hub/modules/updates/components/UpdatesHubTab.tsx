import { Empty, List, Spin, Tag, Typography } from 'antd'
import { Stores } from '@/core/stores'

const { Text } = Typography

/**
 * Installed entities (assistants / MCP servers / models that were
 * created from a hub item) whose `hub_version` lags the current
 * catalog. Admin-only — the underlying endpoint
 * `GET /api/hub/updates` is gated on `hub::admin`.
 */
export function UpdatesHubTab() {
  const updates = Stores.HubUpdates.updates
  const loading = Stores.HubUpdates.loading
  const error = Stores.HubUpdates.error
  const catalogVersion = Stores.HubUpdates.catalogVersion

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

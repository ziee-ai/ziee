import { useEffect } from 'react'
import {
  Badge,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Select,
  Space,
  Tag,
  Typography
} from 'antd'
import { PlayCircleOutlined, PoweroffOutlined, ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { RuntimeEngine } from '../types'

interface Props {
  engine: RuntimeEngine
}

export function RuntimeModelsByVersion({ engine }: Props) {
  const { usage, loading, acting } = Stores.RuntimeModelUsage
  const canManage = usePermission(Permissions.LocalRuntimeManage)

  const data = usage.get(engine)
  const isLoading = loading.get(engine) || false

  useEffect(() => {
    Stores.RuntimeModelUsage.loadUsage(engine)
  }, [engine])

  // All versions of this engine — the swap dropdown's option set.
  const versionOptions = (data?.versions ?? []).map(v => ({
    value: v.version.id,
    label: v.version.is_system_default
      ? `${v.version.version} (${v.version.backend}, default)`
      : `${v.version.version} (${v.version.backend})`
  }))

  return (
    <Card
      size="small"
      title="Models by engine version"
      extra={
        <Button
          icon={<ReloadOutlined />}
          loading={isLoading}
          onClick={() => Stores.RuntimeModelUsage.loadUsage(engine)}
        >
          Refresh
        </Button>
      }
    >
      {!data || (data.versions.length === 0 && data.unresolved.length === 0) ? (
        <Empty description="No installed versions yet" />
      ) : (
        <Flex vertical gap="middle">
          {data.versions.map(entry => (
            <div key={entry.version.id}>
              <Space>
                <Typography.Text strong>{entry.version.version}</Typography.Text>
                <Tag>{entry.version.backend}</Tag>
                {entry.version.is_system_default && <Tag color="success">Default</Tag>}
                <Typography.Text type="secondary">
                  {entry.models.length === 0
                    ? 'No models — safe to delete'
                    : `${entry.models.length} model(s)`}
                </Typography.Text>
              </Space>

              {entry.models.length > 0 && (
                <div style={{ marginTop: 8 }}>
                  {groupByProvider(entry.models).map(group => (
                    <div key={group.providerId} style={{ marginBottom: 8 }}>
                      <Typography.Text type="secondary" style={{ fontSize: 12 }}>
                        {group.providerName}
                      </Typography.Text>
                      {group.models.map(m => {
                        const busy = acting.get(m.id) || false
                        return (
                          <Flex
                            key={m.id}
                            align="center"
                            justify="space-between"
                            gap="small"
                            style={{ padding: '4px 0' }}
                          >
                            <Space>
                              <Badge status={m.running ? 'processing' : 'default'} />
                              <span>{m.display_name}</span>
                              {!m.pinned && <Tag color="default">inherited</Tag>}
                            </Space>
                            {canManage && (
                              <Space>
                                <Select
                                  size="small"
                                  style={{ minWidth: 180 }}
                                  value={entry.version.id}
                                  options={versionOptions}
                                  loading={busy}
                                  disabled={busy || versionOptions.length < 2}
                                  onChange={versionId =>
                                    Stores.RuntimeModelUsage.swapVersion(
                                      engine,
                                      m.id,
                                      versionId
                                    ).catch(() => {})
                                  }
                                />
                                {m.running ? (
                                  <Button
                                    size="small"
                                    danger
                                    icon={<PoweroffOutlined />}
                                    loading={busy}
                                    onClick={() =>
                                      Stores.RuntimeModelUsage.stopModel(engine, m.id).catch(
                                        () => {}
                                      )
                                    }
                                  >
                                    Stop
                                  </Button>
                                ) : (
                                  <Button
                                    size="small"
                                    icon={<PlayCircleOutlined />}
                                    loading={busy}
                                    onClick={() =>
                                      Stores.RuntimeModelUsage.startModel(engine, m.id).catch(
                                        () => {}
                                      )
                                    }
                                  >
                                    Start
                                  </Button>
                                )}
                              </Space>
                            )}
                          </Flex>
                        )
                      })}
                    </div>
                  ))}
                </div>
              )}
              <Divider style={{ margin: '8px 0 0' }} />
            </div>
          ))}

          {data.unresolved.length > 0 && (
            <div>
              <Typography.Text type="warning">
                No installed version resolves for these models:
              </Typography.Text>
              <div style={{ marginTop: 4 }}>
                {data.unresolved.map(m => (
                  <Tag key={m.id}>{m.display_name}</Tag>
                ))}
              </div>
            </div>
          )}
        </Flex>
      )}
    </Card>
  )
}

interface ProviderGroup {
  providerId: string
  providerName: string
  models: Array<{
    id: string
    display_name: string
    running: boolean
    pinned: boolean
  }>
}

function groupByProvider(
  models: Array<{
    id: string
    display_name: string
    provider_id: string
    provider_name: string
    running: boolean
    pinned: boolean
  }>
): ProviderGroup[] {
  const map = new Map<string, ProviderGroup>()
  for (const m of models) {
    let g = map.get(m.provider_id)
    if (!g) {
      g = { providerId: m.provider_id, providerName: m.provider_name, models: [] }
      map.set(m.provider_id, g)
    }
    g.models.push({
      id: m.id,
      display_name: m.display_name,
      running: m.running,
      pinned: m.pinned
    })
  }
  return Array.from(map.values())
}

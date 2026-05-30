import { useEffect, useState } from 'react'
import {
  Badge,
  Button,
  Card,
  Descriptions,
  Divider,
  Empty,
  Flex,
  Select,
  Space,
  Tag,
  Tooltip,
  Typography
} from 'antd'
import {
  DownOutlined,
  PlayCircleOutlined,
  PoweroffOutlined,
  ReloadOutlined,
  UpOutlined
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { RuntimeEngine } from '../types'
import { LiveLogsPanel } from './LiveLogsPanel'

interface Props {
  engine: RuntimeEngine
}

interface ModelInfo {
  id: string
  display_name: string
  running: boolean
  pinned: boolean
}

export function RuntimeModelsByVersion({ engine }: Props) {
  const { usage, loading } = Stores.RuntimeModelUsage
  const canManage = usePermission(Permissions.LocalRuntimeManage)
  // Logs are a distinct backend permission (`llm_local_runtime::logs`), NOT
  // bundled under manage — a logs-only operator should see logs, and a
  // manage-only operator should not see a Logs button that 403s on the stream.
  const canViewLogs = usePermission(Permissions.LocalRuntimeLogs)

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
        <Empty description="No installed versions yet" image={Empty.PRESENTED_IMAGE_SIMPLE} />
      ) : (
        <Flex vertical gap="middle">
          {data.versions.map((entry, idx) => (
            <Flex vertical gap="small" key={entry.version.id}>
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
                <Flex vertical gap="small">
                  {groupByProvider(entry.models).map(group => (
                    <Flex vertical gap="small" key={group.providerId}>
                      <Typography.Text type="secondary">
                        {group.providerName}
                      </Typography.Text>
                      {group.models.map(m => (
                        <ModelRow
                          key={m.id}
                          engine={engine}
                          model={m}
                          versionId={entry.version.id}
                          versionOptions={versionOptions}
                          canManage={canManage}
                          canViewLogs={canViewLogs}
                        />
                      ))}
                    </Flex>
                  ))}
                </Flex>
              )}
              {idx < data.versions.length - 1 && <Divider className="!my-0" />}
            </Flex>
          ))}

          {data.unresolved.length > 0 && (
            <Flex vertical gap="small">
              <Typography.Text type="warning">
                No installed version resolves for these models:
              </Typography.Text>
              <div>
                {data.unresolved.map(m => (
                  <Tag key={m.id}>{m.display_name}</Tag>
                ))}
              </div>
            </Flex>
          )}
        </Flex>
      )}
    </Card>
  )
}

function ModelRow({
  engine,
  model,
  versionId,
  versionOptions,
  canManage,
  canViewLogs
}: {
  engine: RuntimeEngine
  model: ModelInfo
  versionId: string
  versionOptions: { value: string; label: string }[]
  canManage: boolean
  canViewLogs: boolean
}) {
  const { acting, instances } = Stores.RuntimeModelUsage
  const [expanded, setExpanded] = useState(false)
  const busy = acting.get(model.id) || false
  const instance = instances.get(model.id)

  // Lazily fetch instance detail when the row is expanded on a running model.
  useEffect(() => {
    if (expanded && model.running) {
      Stores.RuntimeModelUsage.loadInstance(model.id)
    }
  }, [expanded, model.running, model.id])

  return (
    <Flex vertical gap="small" className="py-1">
      <Flex align="center" justify="space-between" gap="small">
        <Space>
          <Badge status={model.running ? 'processing' : 'default'} />
          <span>{model.display_name}</span>
          {!model.pinned && <Tag color="default">inherited</Tag>}
        </Space>
        <Space>
          {canManage && (
            <>
              <Tooltip
                title={
                  versionOptions.length < 2
                    ? 'Only one engine version installed — download another to swap'
                    : 'Swap this model to a different engine version'
                }
              >
                <Select
                  className="min-w-[180px]"
                  value={versionId}
                  options={versionOptions}
                  loading={busy}
                  disabled={busy || versionOptions.length < 2}
                  onChange={vid =>
                    Stores.RuntimeModelUsage.swapVersion(engine, model.id, vid).catch(() => {})
                  }
                  aria-label={`Engine version for ${model.display_name}`}
                />
              </Tooltip>
              {model.running ? (
                <>
                  <Button
                    icon={<ReloadOutlined />}
                    loading={busy}
                    onClick={() =>
                      Stores.RuntimeModelUsage.restartModel(engine, model.id).catch(() => {})
                    }
                  >
                    Restart
                  </Button>
                  <Button
                    danger
                    icon={<PoweroffOutlined />}
                    loading={busy}
                    onClick={() =>
                      Stores.RuntimeModelUsage.stopModel(engine, model.id).catch(() => {})
                    }
                  >
                    Stop
                  </Button>
                </>
              ) : (
                <Button
                  icon={<PlayCircleOutlined />}
                  loading={busy}
                  onClick={() =>
                    Stores.RuntimeModelUsage.startModel(engine, model.id).catch(() => {})
                  }
                >
                  Start
                </Button>
              )}
            </>
          )}
          {model.running && canViewLogs && (
            <Button
              type="text"
              icon={expanded ? <UpOutlined /> : <DownOutlined />}
              onClick={() => setExpanded(e => !e)}
              aria-label={
                expanded
                  ? `Hide logs for ${model.display_name}`
                  : `Show logs for ${model.display_name}`
              }
              aria-expanded={expanded}
            >
              Logs
            </Button>
          )}
        </Space>
      </Flex>

      {expanded && model.running && (
        <Flex vertical gap="small" className="pl-6">
          {instance && (
            <Descriptions size="small" column={2}>
              <Descriptions.Item label="Status">{instance.status}</Descriptions.Item>
              <Descriptions.Item label="Port">{instance.local_port}</Descriptions.Item>
              <Descriptions.Item label="Base URL">{instance.base_url}</Descriptions.Item>
              <Descriptions.Item label="Started">
                {instance.started_at
                  ? new Date(instance.started_at).toLocaleString()
                  : '—'}
              </Descriptions.Item>
              <Descriptions.Item label="Last health check">
                {instance.last_health_check
                  ? new Date(instance.last_health_check).toLocaleString()
                  : '—'}
              </Descriptions.Item>
              {instance.error_message && (
                <Descriptions.Item label="Error">{instance.error_message}</Descriptions.Item>
              )}
            </Descriptions>
          )}
          <LiveLogsPanel modelId={model.id} />
        </Flex>
      )}
    </Flex>
  )
}

interface ProviderGroup {
  providerId: string
  providerName: string
  models: ModelInfo[]
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

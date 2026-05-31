import { useEffect, useState } from 'react'
import {
  Badge,
  Button,
  Collapse,
  Descriptions,
  Empty,
  Flex,
  Select,
  Space,
  Tag,
  Tooltip,
  Typography,
} from 'antd'
import {
  DownOutlined,
  PlayCircleOutlined,
  PoweroffOutlined,
  ReloadOutlined,
  UpOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import type { RuntimeEngine } from '../types'
import { LiveLogsPanel } from './LiveLogsPanel'

interface ModelInfo {
  id: string
  display_name: string
  running: boolean
  pinned: boolean
}

interface RawModel {
  id: string
  display_name: string
  provider_id: string
  provider_name: string
  running: boolean
  pinned: boolean
}

/**
 * Per-version models block — extracted from the old standalone
 * RuntimeModelsByVersion card so it can render inline under each
 * installed-version row in InstalledVersionsCard. Groups the models
 * by provider, exposes start/stop/restart + version-swap controls
 * (manage gate) + the Logs disclosure (logs gate, independent
 * permission so a logs-only operator still sees them on a running
 * model).
 *
 * Receives `versionOptions` from the parent — the swap dropdown's
 * full set of installed versions for this engine, including the
 * current one (so the Select shows the active value as well).
 */
export function VersionModelsBlock({
  engine,
  versionId,
  models,
  versionOptions,
  canManage,
  canViewLogs,
}: {
  engine: RuntimeEngine
  versionId: string
  models: RawModel[]
  versionOptions: { value: string; label: string }[]
  canManage: boolean
  canViewLogs: boolean
}) {
  const groups = groupByProvider(models)
  const label = (
    <Typography.Text type="secondary" className="text-xs">
      Models using this version ({models.length})
    </Typography.Text>
  )
  return (
    <Collapse
      ghost
      size="small"
      // Default open: an operator scrolling the Installed versions
      // card usually wants to see which models pin each version
      // (especially when deciding whether to delete a version). They
      // can always collapse to tidy up.
      defaultActiveKey="models"
      items={[
        {
          key: 'models',
          label,
          children:
            models.length === 0 ? (
              <Empty
                description="No models use this version — safe to delete"
                image={Empty.PRESENTED_IMAGE_SIMPLE}
              />
            ) : (
              <Flex vertical gap="small">
                {groups.map(group => (
                  <Flex vertical gap="small" key={group.providerId}>
                    <Typography.Text type="secondary" className="text-xs">
                      {group.providerName}
                    </Typography.Text>
                    {group.models.map(m => (
                      <ModelRow
                        key={m.id}
                        engine={engine}
                        model={m}
                        versionId={versionId}
                        versionOptions={versionOptions}
                        canManage={canManage}
                        canViewLogs={canViewLogs}
                      />
                    ))}
                  </Flex>
                ))}
              </Flex>
            ),
        },
      ]}
    />
  )
}

function ModelRow({
  engine,
  model,
  versionId,
  versionOptions,
  canManage,
  canViewLogs,
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
                    ? 'Only one engine version installed — install another to swap'
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
                    Stores.RuntimeModelUsage.swapVersion(engine, model.id, vid).catch(
                      () => {},
                    )
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
                      Stores.RuntimeModelUsage.restartModel(engine, model.id).catch(
                        () => {},
                      )
                    }
                  >
                    Restart
                  </Button>
                  <Button
                    danger
                    icon={<PoweroffOutlined />}
                    loading={busy}
                    onClick={() =>
                      Stores.RuntimeModelUsage.stopModel(engine, model.id).catch(
                        () => {},
                      )
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
                    Stores.RuntimeModelUsage.startModel(engine, model.id).catch(
                      () => {},
                    )
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

function groupByProvider(models: RawModel[]): ProviderGroup[] {
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
      pinned: m.pinned,
    })
  }
  return Array.from(map.values())
}

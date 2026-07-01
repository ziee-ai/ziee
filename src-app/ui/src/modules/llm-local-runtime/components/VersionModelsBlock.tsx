import { useEffect, useState } from 'react'
import {
  Accordion,
  Button,
  Descriptions,
  Empty,
  Flex,
  Select,
  Space,
  Tag,
  Text,
  Tooltip,
} from '@/components/ui'
import { ChevronDown, CirclePlay, Power, RotateCw, ChevronUp } from 'lucide-react'
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
    <Text type="secondary" className="text-xs">
      Models using this version ({models.length})
    </Text>
  )
  return (
    <Accordion
      ghost
      defaultValue="models"
      data-testid={`llmrt-version-models-${versionId}`}
      items={[
        {
          key: 'models',
          label,
          children:
            models.length === 0 ? (
              <Empty
                description="No models use this version — safe to delete"
                data-testid={`llmrt-version-models-empty-${versionId}`}
              />
            ) : (
              <Flex direction="column" gap="small">
                {groups.map(group => (
                  <Flex direction="column" gap="small" key={group.providerId}>
                    <Text type="secondary" className="text-xs">
                      {group.providerName}
                    </Text>
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
    <Flex direction="column" gap="small" className="py-1" data-testid={`llmrt-model-row-${model.id}`}>
      <Flex align="center" justify="between" gap="small">
        <Space>
          <span className={`inline-block size-2 rounded-full ${model.running ? 'bg-primary' : 'bg-muted-foreground/40'}`} aria-hidden />
          <span>{model.display_name}</span>
          {!model.pinned && <Tag variant="outline" tone="default" data-testid={`llmrt-model-inherited-tag-${model.id}`}>inherited</Tag>}
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
                  data-testid={`llmrt-model-version-select-${model.id}`}
                  value={versionId}
                  options={versionOptions}
                  loading={busy}
                  disabled={busy || versionOptions.length < 2}
                  onChange={vid =>
                    Stores.RuntimeModelUsage.swapVersion(engine, model.id, vid).catch(
                      () => {},
                    )
                  }
                  aria-label={
                    versionOptions.length < 2
                      ? `Engine version for ${model.display_name} — swapping disabled, only one engine version installed; install another to swap`
                      : `Engine version for ${model.display_name}`
                  }
                />
              </Tooltip>
              {model.running ? (
                <>
                  <Button
                    icon={<RotateCw />}
                    data-testid={`llmrt-model-restart-${model.id}`}
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
                    variant="destructive"
                    icon={<Power />}
                    data-testid={`llmrt-model-stop-${model.id}`}
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
                  icon={<CirclePlay />}
                  data-testid={`llmrt-model-start-${model.id}`}
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
              variant="ghost"
              data-testid={`llmrt-model-logs-${model.id}`}
              icon={expanded ? <ChevronUp /> : <ChevronDown />}
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
        <Flex direction="column" gap="small" className="pl-6">
          {instance && (
            <Descriptions
              size="sm"
              column={2}
              data-testid={`llmrt-model-instance-desc-${model.id}`}
              items={[
                { key: 'status', label: 'Status', children: instance.status },
                { key: 'port', label: 'Port', children: instance.local_port },
                { key: 'baseUrl', label: 'Base URL', children: instance.base_url },
                {
                  key: 'started',
                  label: 'Started',
                  children: instance.started_at
                    ? new Date(instance.started_at).toLocaleString()
                    : '—',
                },
                {
                  key: 'health',
                  label: 'Last health check',
                  children: instance.last_health_check
                    ? new Date(instance.last_health_check).toLocaleString()
                    : '—',
                },
                ...(instance.error_message
                  ? [{ key: 'error', label: 'Error', children: instance.error_message }]
                  : []),
              ]}
            />
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

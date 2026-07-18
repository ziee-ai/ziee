import { Power, RotateCw } from 'lucide-react'
import { useState } from 'react'
import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/types'
import {
  Button,
  Card,
  Descriptions,
  Empty,
  ErrorState,
  Flex,
  message,
  Spin,
  Tag,
  Text,
} from '@ziee/kit'
import { Can, usePermission } from '@/core/permissions'
import { Stores } from '@ziee/framework/stores'

/** Format a duration in seconds as `1h 02m 03s`. */
function formatUptime(secs: number): string {
  const h = Math.floor(secs / 3600)
  const m = Math.floor((secs % 3600) / 60)
  const s = Math.floor(secs % 60)
  const parts: string[] = []
  if (h > 0) parts.push(`${h}h`)
  if (h > 0 || m > 0) parts.push(`${String(m).padStart(2, '0')}m`)
  parts.push(`${String(s).padStart(2, '0')}s`)
  return parts.join(' ')
}

/**
 * Health of the single managed whisper-server instance: coarse status +
 * fine health-state + live pid/uptime, with restart/stop controls and a
 * captured-log viewer.
 */
export function VoiceInstanceCard() {
  const { info, loading, busy, error } = Stores.VoiceInstance

  const handleRestart = async () => {
    try {
      await Stores.VoiceInstance.restartInstance()
      message.success('Instance restarting')
    } catch (e) {
      message.error(
        e instanceof Error ? e.message : 'Failed to restart instance',
      )
    }
  }

  const handleStop = async () => {
    try {
      await Stores.VoiceInstance.stopInstance()
      message.success('Instance stopped')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to stop instance')
    }
  }

  const running = info?.status === 'running'

  return (
    <Card
      title="Instance health"
      data-testid="voice-instance-card"
      extra={
        <Can permission={Permissions.VoiceAdminManage}>
          <Flex gap="small">
            <Button
              icon={<RotateCw />}
              loading={busy}
              onClick={handleRestart}
              data-testid="voice-instance-restart-btn"
              aria-label="Restart voice instance"
            >
              Restart
            </Button>
            <Button
              icon={<Power />}
              variant="outline"
              loading={busy}
              disabled={!running}
              onClick={handleStop}
              data-testid="voice-instance-stop-btn"
              aria-label="Stop voice instance"
            >
              Stop
            </Button>
          </Flex>
        </Can>
      }
    >
      {loading && !info ? (
        <Spin label="Loading" />
      ) : error && !info ? (
        <ErrorState
          resource="instance"
          description="The instance status couldn't be loaded."
          details={error}
          onRetry={() => Stores.VoiceInstance.loadInstance()}
          data-testid="voice-instance-error"
        />
      ) : !info ? (
        <Descriptions
          size="sm"
          data-testid="voice-instance-desc"
          items={[{ key: 'status', label: 'Status', children: 'Unknown' }]}
        />
      ) : (
        <Flex vertical gap="small">
          <Flex
            align="center"
            gap="small"
            wrap
            data-testid="voice-instance-status-row"
          >
            <Tag
              tone={running ? 'success' : undefined}
              variant="outline"
              data-testid="voice-instance-status-tag"
            >
              {info.status}
            </Tag>
            <Tag variant="outline" data-testid="voice-instance-state-tag">
              {info.state}
            </Tag>
          </Flex>
          <Descriptions
            size="sm"
            data-testid="voice-instance-desc"
            items={[
              ...(info.active_model
                ? [
                    {
                      key: 'model',
                      label: 'Active model',
                      children: info.active_model,
                    },
                  ]
                : []),
              ...(info.local_port != null
                ? [
                    {
                      key: 'port',
                      label: 'Port',
                      children: String(info.local_port),
                    },
                  ]
                : []),
              ...(info.pid != null
                ? [{ key: 'pid', label: 'PID', children: String(info.pid) }]
                : []),
              ...(info.uptime_seconds != null
                ? [
                    {
                      key: 'uptime',
                      label: 'Uptime',
                      children: formatUptime(info.uptime_seconds),
                    },
                  ]
                : []),
              {
                key: 'restarts',
                label: 'Restart attempts',
                children: String(info.restart_attempts),
              },
              ...(info.last_failure_reason
                ? [
                    {
                      key: 'failure',
                      label: 'Last failure',
                      children: info.last_failure_reason,
                    },
                  ]
                : []),
              ...(info.last_used_at
                ? [
                    {
                      key: 'used',
                      label: 'Last used',
                      children: new Date(info.last_used_at).toLocaleString(),
                    },
                  ]
                : []),
            ]}
          />
          <InstanceLogs />
        </Flex>
      )}
    </Card>
  )
}

/**
 * Captured whisper-server log lines (ring buffer). Fetched on demand via a
 * refresh button — transient UI state, so local `useState` (no store). Gated on
 * VoiceAdminRead like the rest of the page.
 */
function InstanceLogs() {
  const canRead = usePermission(Permissions.VoiceAdminRead)
  const [lines, setLines] = useState<string[] | null>(null)
  const [loadingLogs, setLoadingLogs] = useState(false)

  if (!canRead) return null

  const handleRefresh = async () => {
    setLoadingLogs(true)
    try {
      const resp = await ApiClient.Voice.getInstanceLogs({ lines: 200 })
      setLines(resp.lines)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to load logs')
    } finally {
      setLoadingLogs(false)
    }
  }

  return (
    <div className="mt-2" data-testid="voice-instance-logs">
      <Flex justify="between" align="center" gap="small" wrap className="mb-2">
        <Text strong className="text-xs">
          Logs
        </Text>
        <Button
          icon={<RotateCw />}
          size="default"
          variant="outline"
          loading={loadingLogs}
          onClick={handleRefresh}
          data-testid="voice-instance-logs-refresh"
          aria-label="Refresh instance logs"
        >
          {lines == null ? 'Load logs' : 'Refresh'}
        </Button>
      </Flex>
      {lines == null ? (
        <Text type="secondary" className="text-xs">
          Load the most recent whisper-server output.
        </Text>
      ) : lines.length === 0 ? (
        <Empty
          description="No log output captured yet"
          data-testid="voice-instance-logs-empty"
        />
      ) : (
        <div
          className="max-h-[320px] overflow-y-auto rounded border border-border bg-muted p-2 font-mono text-xs whitespace-pre-wrap break-all"
          data-testid="voice-instance-logs-block"
        >
          {lines.map((line, i) => (
            <div key={i}>{line}</div>
          ))}
        </div>
      )}
    </div>
  )
}

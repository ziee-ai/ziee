import { Power, RotateCw } from 'lucide-react'
import {
  Button,
  Card,
  Descriptions,
  ErrorState,
  Flex,
  Spin,
  Tag,
  message,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'

/**
 * Health of the single managed whisper-server instance: coarse status +
 * fine health-state, with restart/stop controls.
 */
export function VoiceInstanceCard() {
  const { info, loading, busy, error } = Stores.VoiceInstance

  const handleRestart = async () => {
    try {
      await Stores.VoiceInstance.restartInstance()
      message.success('Instance restarting')
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to restart instance')
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
          <Flex align="center" gap="small" wrap data-testid="voice-instance-status-row">
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
                ? [{ key: 'model', label: 'Active model', children: info.active_model }]
                : []),
              ...(info.local_port != null
                ? [{ key: 'port', label: 'Port', children: String(info.local_port) }]
                : []),
              { key: 'restarts', label: 'Restart attempts', children: String(info.restart_attempts) },
              ...(info.last_failure_reason
                ? [{ key: 'failure', label: 'Last failure', children: info.last_failure_reason }]
                : []),
              ...(info.last_used_at
                ? [{
                    key: 'used',
                    label: 'Last used',
                    children: new Date(info.last_used_at).toLocaleString(),
                  }]
                : []),
            ]}
          />
        </Flex>
      )}
    </Card>
  )
}

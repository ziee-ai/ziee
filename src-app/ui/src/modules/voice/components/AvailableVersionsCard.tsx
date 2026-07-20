import { Download, RotateCw } from 'lucide-react'
import { useEffect } from 'react'
import {
  Button,
  Card,
  ErrorState,
  Flex,
  Progress,
  Separator,
  Space,
  Spin,
  Tag,
  Text,
  message,
} from '@ziee/kit'
import { Stores } from '@ziee/framework/stores'
import { Can } from '@/core/permissions'
import { type AvailableVersion2, type DownloadSnapshot2 } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'

/** Human-readable byte sizes. */
function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/**
 * Available whisper runtimes — upstream releases installable for this host,
 * with a live Install progress bar driven by the SSE download store. Reload-safe
 * (VoiceDownloadProgress.init re-attaches to in-flight tasks). Mirrors
 * llm-local-runtime's AvailableVersionsCard, single-engine.
 */
export function AvailableVersionsCard() {
  const { updateCheck, checking, error } = Stores.VoiceUpdate
  const { activeByKey } = Stores.VoiceDownloadProgress

  const progressKey = (v: AvailableVersion2) => {
    const backend = v.recommended_backend ?? v.available_backends?.[0] ?? 'cpu'
    return `whisper@${v.version}@${backend}`
  }

  useEffect(() => {
    if (!updateCheck && !checking) {
      Stores.VoiceUpdate.checkForUpdates().catch(() => {})
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  const platform = updateCheck?.platform
  const arch = updateCheck?.arch
  const readyUpstream = (updateCheck?.versions ?? []).filter(v => v.binary_ready)
  const latestVersion = readyUpstream[0]?.version ?? updateCheck?.versions?.[0]?.version

  const handleDownload = async (v: AvailableVersion2) => {
    const backend = v.recommended_backend ?? v.available_backends?.[0] ?? 'cpu'
    try {
      await Stores.VoiceDownloadProgress.startDownload({
        version: v.version,
        platform,
        arch,
        backend,
      })
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to start download')
    }
  }

  const handleCheckForUpdates = async () => {
    try {
      const result = await Stores.VoiceUpdate.checkForUpdates()
      const ready = (result?.versions ?? []).filter(v => v.binary_ready)
      const newCount = ready.filter(v => !v.installed).length
      if (newCount === 0) {
        message.success("No new runtimes — you're up to date.")
      } else {
        message.success(`Found ${newCount} new ${newCount === 1 ? 'runtime' : 'runtimes'}.`)
      }
    } catch (e) {
      message.error(
        e instanceof Error ? `Update check failed: ${e.message}` : 'Update check failed',
      )
    }
  }

  return (
    <Card
      title="Available runtimes"
      data-testid="voice-available-versions-card"
      extra={
        <Can permission={Permissions.VoiceAdminRead}>
          <Button
            icon={<RotateCw />}
            loading={checking}
            onClick={handleCheckForUpdates}
            data-testid="voice-check-updates-btn"
            aria-label="Check for updates"
          >
            Check for updates
          </Button>
        </Can>
      }
    >
      <Flex vertical className="gap-4">
        {platform && arch && (
          <div data-testid="voice-platform-row">
            <Text type="secondary">Platform: </Text>
            <Text strong>
              {platform}/{arch}
            </Text>
          </div>
        )}

        <Separator className="!my-2" />

        {checking && !updateCheck ? (
          <Spin label="Checking for updates" />
        ) : error && !updateCheck ? (
          <ErrorState
            resource="available runtimes"
            description="Couldn't reach the upstream release feed."
            details={error}
            onRetry={() => void Stores.VoiceUpdate.checkForUpdates().catch(() => {})}
            data-testid="voice-available-error"
          />
        ) : !updateCheck ? (
          <Text type="secondary">Could not reach the upstream release feed.</Text>
        ) : readyUpstream.length === 0 ? (
          <Text type="secondary">
            No published binaries found{platform && arch ? ` for ${platform}/${arch}` : ''}.
          </Text>
        ) : (
          <Flex vertical gap="small">
            {readyUpstream.slice(0, 10).map(v => (
              <AvailableVersionRow
                key={v.version}
                v={v}
                isLatest={v.version === latestVersion}
                progress={activeByKey.get(progressKey(v))}
                onDownload={() => handleDownload(v)}
              />
            ))}
            {readyUpstream.length > 10 && (
              <Text type="secondary">
                +{readyUpstream.length - 10} older versions hidden
              </Text>
            )}
          </Flex>
        )}
      </Flex>
    </Card>
  )
}

function AvailableVersionRow({
  v,
  isLatest,
  progress,
  onDownload,
}: {
  v: AvailableVersion2
  isLatest: boolean
  progress?: DownloadSnapshot2
  onDownload: () => void
}) {
  const inProgress =
    progress != null && progress.status !== 'completed' && progress.status !== 'failed'
  const failed = progress?.status === 'failed'
  return (
    <div
      className="rounded -mx-2 px-2 -my-1 py-1"
      data-testid={`voice-version-row-${v.version}`}
    >
      <Flex vertical gap="small">
        <Flex justify="between" align="center" gap="small" wrap>
          <Space wrap>
            <Text strong>{v.version}</Text>
            {v.size_bytes != null && !v.installed && (
              <Text type="secondary" className="text-xs">
                {formatBytes(v.size_bytes)}
              </Text>
            )}
            {isLatest && (
              <Tag tone="info" variant="outline" data-testid={`voice-version-latest-tag-${v.version}`}>
                latest
              </Tag>
            )}
            {v.installed && (
              <Tag tone="success" variant="outline" data-testid={`voice-version-installed-tag-${v.version}`}>
                installed
              </Tag>
            )}
            {v.prerelease && (
              <Tag variant="outline" data-testid={`voice-version-prerelease-tag-${v.version}`}>
                prerelease
              </Tag>
            )}
          </Space>
          <Can permission={Permissions.VoiceAdminManage}>
            <Button
              icon={<Download />}
              loading={inProgress}
              disabled={v.installed || inProgress}
              onClick={onDownload}
              data-testid={`voice-version-install-${v.version}`}
              aria-label={`Install ${v.version}`}
            >
              {v.installed ? 'Installed' : inProgress ? 'Installing…' : 'Install'}
            </Button>
          </Can>
        </Flex>
        {progress && <DownloadProgressLine progress={progress} />}
        {failed && progress?.error && <Text type="secondary">{progress.error}</Text>}
      </Flex>
    </div>
  )
}

function DownloadProgressLine({ progress }: { progress: DownloadSnapshot2 }) {
  const total = progress.total_bytes ?? 0
  const recv = progress.bytes_received
  const pct =
    progress.status === 'completed'
      ? 100
      : progress.percent != null
      ? Math.round(progress.percent)
      : total > 0
      ? Math.round((recv / total) * 100)
      : undefined
  return (
    <Flex vertical className="gap-1">
      <Progress
        value={pct ?? 0}
        data-testid={`voice-download-progress-${progress.key}`}
        tone={
          progress.status === 'failed'
            ? 'error'
            : progress.status === 'completed'
            ? 'success'
            : 'primary'
        }
        showInfo={pct != null}
        size="sm"
        aria-label={`Download progress: ${pct ?? 0}%`}
      />
      <Text type="secondary" className="text-xs">
        {formatBytes(recv)}
        {total > 0 ? ` / ${formatBytes(total)}` : ''}
        {progress.status === 'completed' ? ' — Completed' : ''}
      </Text>
    </Flex>
  )
}

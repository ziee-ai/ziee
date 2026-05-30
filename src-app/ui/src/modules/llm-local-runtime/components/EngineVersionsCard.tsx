import { Fragment, useEffect, type ReactNode } from 'react'
import {
  App,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Progress,
  Space,
  Spin,
  Tag,
  theme,
  Typography,
} from 'antd'
import { DownloadOutlined, ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { DownloadSnapshot } from '@/api-client/types'
import type { RuntimeEngine, RuntimeAvailableVersion } from '../types'
import { RuntimeVersionCard } from './RuntimeVersionCard'

const { Text } = Typography

const BACKEND_LABEL: Record<string, string> = {
  cpu: 'CPU',
  cuda: 'NVIDIA CUDA',
  metal: 'Apple Metal',
  rocm: 'AMD ROCm',
  vulkan: 'Vulkan',
  opencl: 'OpenCL',
}

/**
 * One unified card per engine that consolidates the three previous
 * cards (GpuDetectionCard + RuntimeUpdateChecker + RuntimeVersionList)
 * into the four sections the user asked for:
 *
 *   1. Platform              — e.g. linux/x86_64
 *   2. Available backends    — CPU, CUDA, Metal …  (recommended highlighted)
 *   3. Installed versions    — list of installed binaries with Set-default / Delete
 *   4. Available versions    — upstream releases auto-checked on mount,
 *                              each with an inline Download button
 *
 * No "Check for updates" button — the update check runs automatically
 * on mount (if not already cached). No standalone Download drawer
 * trigger — per-version download buttons pick the recommended backend
 * for the host. The drawer remains available for the advanced case
 * (custom version string / non-default backend) but isn't on the
 * happy path anymore.
 */
export function EngineVersionsCard({ engine }: { engine: RuntimeEngine }) {
  const { gpu, loadingGpu } = Stores.RuntimeConfig
  const { versions, loading: loadingVersions } = Stores.RuntimeVersion
  const { updateChecks, checking } = Stores.RuntimeUpdate
  const { activeByKey } = Stores.RuntimeDownloadProgress
  const { message } = App.useApp()

  const updateCheck = updateChecks.get(engine)
  const isChecking = checking.get(engine) || false

  const engineVersions = versions.filter(v => v.engine === engine)
  // The progress-store keys by `engine@version@backend`; for the
  // available-version row we don't know the backend up-front, so
  // resolve via the same fallback `handleDownload` uses.
  const progressKey = (v: RuntimeAvailableVersion) => {
    const backend = v.recommended_backend ?? v.available_backends[0] ?? 'cpu'
    return `${engine}@${v.version}@${backend}`
  }

  // Auto-load gpu + versions + update check on mount.
  useEffect(() => {
    if (!gpu && !loadingGpu) {
      Stores.RuntimeConfig.loadGpu().catch(() => {})
    }
    if (versions.length === 0 && !loadingVersions) {
      Stores.RuntimeVersion.loadVersions().catch(() => {})
    }
    if (!updateCheck && !isChecking) {
      Stores.RuntimeUpdate.checkForUpdates(engine).catch(() => {
        // Surfaced via the store; the section just shows "couldn't check".
      })
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine])

  const platform = updateCheck?.platform ?? gpu?.platform
  const arch = updateCheck?.arch ?? gpu?.arch

  // Only show binaries that are actually published for this host
  // (filters out tags whose release pipeline is still building).
  const readyUpstream = (updateCheck?.versions ?? []).filter(
    v => v.binary_ready,
  )

  const handleDownload = async (v: RuntimeAvailableVersion) => {
    if (!platform || !arch) {
      message.error(
        'Host platform/arch not detected yet — try again in a moment.',
      )
      return
    }
    const backend = v.recommended_backend ?? v.available_backends[0] ?? 'cpu'
    try {
      // Detached on the server: this returns as soon as the task is
      // registered; the SSE subscription opened by the store drives
      // the progress bar from here on. A page reload re-attaches via
      // the store's loadActive() on mount, so the bar survives.
      await Stores.RuntimeDownloadProgress.startDownload({
        engine,
        version: v.version,
        platform,
        arch,
        backend,
      })
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Failed to start download')
    }
  }

  return (
    <Card title={`${engine} versions`}>
      <Flex vertical gap="middle">
        {/* Platform */}
        <PlatformRow gpu={gpu} loadingGpu={loadingGpu} />

        {/* Available backends */}
        <BackendsRow gpu={gpu} loadingGpu={loadingGpu} />

        <Divider className="!my-2" />

        {/* Installed versions */}
        <Flex vertical gap="small">
          <Text strong>Installed versions</Text>
          {loadingVersions && engineVersions.length === 0 ? (
            <Spin />
          ) : engineVersions.length === 0 ? (
            <Empty
              description="No versions installed yet — download one below."
              image={Empty.PRESENTED_IMAGE_SIMPLE}
            />
          ) : (
            <Flex vertical gap="small">
              {engineVersions.map((v, i) => (
                <Fragment key={v.id}>
                  {i > 0 && <Divider className="!my-2" />}
                  <HoverRow>
                    <RuntimeVersionCard version={v} />
                  </HoverRow>
                </Fragment>
              ))}
            </Flex>
          )}
        </Flex>

        <Divider className="!my-2" />

        {/* Available versions (upstream) — auto-checked on mount; the
            inline button re-runs the check for operators on a flaky
            network or after a release they expect lands. */}
        <Flex vertical gap="small">
          <Flex justify="space-between" align="center" gap="small" wrap>
            <Text strong>Available versions</Text>
            <Button
              icon={<ReloadOutlined />}
              loading={isChecking}
              onClick={async () => {
                try {
                  const result =
                    await Stores.RuntimeUpdate.checkForUpdates(engine)
                  // The store resolves to the new RuntimeUpdateCheck;
                  // surface the latest available version (or a clean
                  // "up to date" if everything ready is installed).
                  const readyAfter = (result?.versions ?? []).filter(
                    rv => rv.binary_ready,
                  )
                  const newCount = readyAfter.filter(rv => !rv.installed).length
                  if (newCount === 0) {
                    message.success(
                      `No new ${engine} versions — you're up to date.`,
                    )
                  } else {
                    message.success(
                      `Found ${newCount} new ${engine} ${
                        newCount === 1 ? 'version' : 'versions'
                      }.`,
                    )
                  }
                } catch (e) {
                  message.error(
                    e instanceof Error
                      ? `Update check failed: ${e.message}`
                      : 'Update check failed',
                  )
                }
              }}
              aria-label={`Check for updates for ${engine}`}
            >
              Check for updates
            </Button>
          </Flex>
          {isChecking && !updateCheck ? (
            <Spin />
          ) : !updateCheck ? (
            <Text type="secondary">
              Could not reach the upstream release feed.
            </Text>
          ) : readyUpstream.length === 0 ? (
            <Text type="secondary">
              No published binaries found for {platform}/{arch}.
            </Text>
          ) : (
            <Flex vertical gap="small">
              {readyUpstream.slice(0, 10).map(v => (
                <AvailableVersionRow
                  key={v.version}
                  v={v}
                  isLatest={v.version === updateCheck.latest_version}
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
      </Flex>
    </Card>
  )
}

function PlatformRow({
  gpu,
  loadingGpu,
}: {
  gpu: ReturnType<typeof useGpu>
  loadingGpu: boolean
}) {
  if (loadingGpu && !gpu) {
    return (
      <div>
        <Text type="secondary">Platform: </Text>
        <Spin size="small" />
      </div>
    )
  }
  if (!gpu) {
    return null
  }
  return (
    <div>
      <Text type="secondary">Platform: </Text>
      <Text strong>
        {gpu.platform}/{gpu.arch}
      </Text>
    </div>
  )
}

function BackendsRow({
  gpu,
  loadingGpu,
}: {
  gpu: ReturnType<typeof useGpu>
  loadingGpu: boolean
}) {
  if (loadingGpu && !gpu) {
    return (
      <Flex align="center" gap="small" wrap>
        <Text type="secondary">Available backends:</Text>
        <Spin size="small" />
      </Flex>
    )
  }
  if (!gpu) {
    return null
  }
  return (
    <Flex align="center" gap="small" wrap>
      <Text type="secondary">Available backends:</Text>
      <Space size={[8, 8]} wrap>
        {gpu.available.map(b => (
          <Tag
            key={b}
            variant="filled"
            color={b === gpu.recommended ? 'green' : 'default'}
          >
            {BACKEND_LABEL[b] ?? b}
          </Tag>
        ))}
      </Space>
    </Flex>
  )
}

// Type helper that mirrors what RuntimeConfig.store exposes for `gpu`.
type GpuShape = NonNullable<ReturnType<typeof useGpu>>
function useGpu() {
  return Stores.RuntimeConfig.gpu
}

function AvailableVersionRow({
  v,
  isLatest,
  progress,
  onDownload,
}: {
  v: RuntimeAvailableVersion
  isLatest: boolean
  /** Live progress snapshot for this row's (engine, version, backend),
   *  or `undefined` when no download is active. */
  progress?: DownloadSnapshot
  onDownload: () => void
}) {
  const inProgress =
    progress != null &&
    progress.status !== 'completed' &&
    progress.status !== 'failed'
  const failed = progress?.status === 'failed'
  return (
    <HoverRow>
      <Flex vertical gap="small">
        <Flex justify="space-between" align="center" gap="small" wrap>
          <Space wrap>
            <Text strong>{v.version}</Text>
            {isLatest && <Tag color="blue" variant="filled">latest</Tag>}
            {v.installed && <Tag color="green" variant="filled">installed</Tag>}
            {v.prerelease && <Tag variant="filled">prerelease</Tag>}
          </Space>
          <Can permission={Permissions.RuntimeVersionCreate}>
            <Button
              icon={<DownloadOutlined />}
              loading={inProgress}
              disabled={v.installed || inProgress}
              onClick={onDownload}
              aria-label={`Download ${v.version}`}
            >
              {v.installed
                ? 'Installed'
                : inProgress
                ? 'Downloading…'
                : 'Download'}
            </Button>
          </Can>
        </Flex>
        {progress && (
          <DownloadProgressLine progress={progress} />
        )}
        {failed && progress?.error && (
          <Text type="danger">{progress.error}</Text>
        )}
      </Flex>
    </HoverRow>
  )
}

/** Inline progress bar + bytes/percent line under an in-flight row. */
function DownloadProgressLine({ progress }: { progress: DownloadSnapshot }) {
  const total = progress.total_bytes ?? 0
  const recv = progress.bytes_received
  const pct =
    progress.percent != null
      ? Math.round(progress.percent)
      : total > 0
      ? Math.round((recv / total) * 100)
      : undefined
  return (
    <Flex vertical gap={4}>
      <Progress
        percent={pct ?? 0}
        status={
          progress.status === 'failed'
            ? 'exception'
            : progress.status === 'completed'
            ? 'success'
            : 'active'
        }
        // Indeterminate-looking when total_bytes is unknown: keep
        // the bar at 0% and rely on the byte counter for feedback.
        showInfo={pct != null}
        size="small"
      />
      <Text type="secondary" className="text-xs">
        {formatBytes(recv)}
        {total > 0 ? ` / ${formatBytes(total)}` : ''}
        {progress.status === 'completed' ? ' — Completed' : ''}
      </Text>
    </Flex>
  )
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  if (n < 1024 * 1024 * 1024) return `${(n / (1024 * 1024)).toFixed(1)} MB`
  return `${(n / (1024 * 1024 * 1024)).toFixed(2)} GB`
}

/**
 * Subtle hover background for list rows, themed via antd's design
 * tokens (matches the AssistantMenuItem / FileAttachMenuItem pattern
 * the chat module uses). `colorFillTertiary` is the right step for a
 * row hover — slightly more present than Quaternary, less than
 * Secondary (which the codebase reserves for menu items). The negative
 * inset + padding lets the highlight visually extend to the Card
 * body's inner padding edge.
 */
function HoverRow({ children }: { children: ReactNode }) {
  const { token } = theme.useToken()
  return (
    <div
      className="rounded -mx-2 px-2 -my-1 py-1"
      style={{ transition: `background-color ${token.motionDurationMid}` }}
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = token.colorFillTertiary
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
      }}
    >
      {children}
    </div>
  )
}

// Suppress unused-locals for the type alias retained for clarity.
export type _GpuShape = GpuShape

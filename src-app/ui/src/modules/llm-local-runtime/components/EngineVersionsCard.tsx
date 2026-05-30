import { useEffect, useState } from 'react'
import {
  App,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Space,
  Spin,
  Tag,
  Typography,
} from 'antd'
import { DownloadOutlined, ReloadOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
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
  const { message } = App.useApp()

  const updateCheck = updateChecks.get(engine)
  const isChecking = checking.get(engine) || false

  const engineVersions = versions.filter(v => v.engine === engine)
  const installedKey = (v: RuntimeAvailableVersion) =>
    `${engine}@${v.version}`
  const [downloadingKeys, setDownloadingKeys] = useState<Set<string>>(
    new Set(),
  )

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
    const key = installedKey(v)
    setDownloadingKeys(prev => new Set(prev).add(key))
    try {
      await Stores.RuntimeVersion.downloadVersion({
        engine,
        version: v.version,
        platform,
        arch,
        backend,
      })
      message.success(`Downloaded ${engine} ${v.version} (${backend})`)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Download failed')
    } finally {
      setDownloadingKeys(prev => {
        const next = new Set(prev)
        next.delete(key)
        return next
      })
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
                <div key={v.id}>
                  {i > 0 && <Divider className="!my-2" />}
                  <RuntimeVersionCard version={v} />
                </div>
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
              onClick={() =>
                Stores.RuntimeUpdate.checkForUpdates(engine).catch(() => {})
              }
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
                  downloading={downloadingKeys.has(installedKey(v))}
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
            bordered
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
  downloading,
  onDownload,
}: {
  v: RuntimeAvailableVersion
  isLatest: boolean
  downloading: boolean
  onDownload: () => void
}) {
  return (
    <Flex justify="space-between" align="center" gap="small" wrap>
      <Space wrap>
        <Text strong>{v.version}</Text>
        {isLatest && <Tag color="blue" bordered>latest</Tag>}
        {v.installed && <Tag color="green" bordered>installed</Tag>}
        {v.prerelease && <Tag bordered>prerelease</Tag>}
      </Space>
      <Can permission={Permissions.RuntimeVersionCreate}>
        <Button
          icon={<DownloadOutlined />}
          loading={downloading}
          disabled={v.installed}
          onClick={onDownload}
          aria-label={`Download ${v.version}`}
        >
          {v.installed ? 'Installed' : 'Download'}
        </Button>
      </Can>
    </Flex>
  )
}

// Suppress unused-locals for the type alias retained for clarity.
export type _GpuShape = GpuShape

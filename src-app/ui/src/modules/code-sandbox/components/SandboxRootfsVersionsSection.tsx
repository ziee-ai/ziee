import { useMemo } from 'react'
import {
  Alert,
  Button,
  Card,
  Divider,
  Empty,
  Flex,
  Modal,
  Popconfirm,
  Progress,
  Spin,
  Tag,
  Tooltip,
  Typography,
} from 'antd'
import {
  CheckCircleTwoTone,
  CloudDownloadOutlined,
  DeleteOutlined,
  PushpinOutlined,
  ReloadOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import {
  Permissions,
  type DrainEntry,
  type InstallTaskState,
  type RootfsArtifact,
  type RootfsRelease,
} from '@/api-client/types'

const { Text } = Typography

const MANAGE_PERM = Permissions.CodeSandboxEnvironmentsManage
const READ_PERM = Permissions.CodeSandboxEnvironmentsRead

const DEFAULT_ARCH = 'x86_64'
const DEFAULT_FLAVORS = ['minimal', 'full']
const DEFAULT_PACKAGE = 'squashfs'

interface Row {
  version: string
  arch: string
  flavor: string
  pkg: string
  artifact?: RootfsArtifact
  release?: RootfsRelease
}

/**
 * Build the visible row set as the union of:
 *   1. every downloaded artifact (`installed`), keyed by
 *      `(version, arch, flavor, package)`, AND
 *   2. for each release in `available` (GitHub catalog), the synthetic
 *      `(version, x86_64, {minimal, full}, squashfs)` rows so the
 *      admin can download what isn't installed yet.
 *
 * Sorts descending by semver version so the newest release floats up.
 */
function buildRows(
  installed: RootfsArtifact[],
  available: RootfsRelease[],
): Row[] {
  const map = new Map<string, Row>()
  for (const a of installed) {
    const key = `${a.version}::${a.arch}::${a.flavor}::${a.package}`
    map.set(key, {
      version: a.version,
      arch: a.arch,
      flavor: a.flavor,
      pkg: a.package,
      artifact: a,
    })
  }
  for (const r of available) {
    if (r.draft || r.prerelease) continue
    for (const flavor of DEFAULT_FLAVORS) {
      const key = `${r.version}::${DEFAULT_ARCH}::${flavor}::${DEFAULT_PACKAGE}`
      if (!map.has(key)) {
        map.set(key, {
          version: r.version,
          arch: DEFAULT_ARCH,
          flavor,
          pkg: DEFAULT_PACKAGE,
          release: r,
        })
      } else {
        const existing = map.get(key)!
        existing.release = r
      }
    }
  }
  return Array.from(map.values()).sort((a, b) => {
    const av = parseSemver(a.version)
    const bv = parseSemver(b.version)
    for (let i = 0; i < 3; i++) {
      if (bv[i] !== av[i]) return bv[i] - av[i]
    }
    return a.flavor.localeCompare(b.flavor)
  })
}

function parseSemver(v: string): [number, number, number] {
  const parts = v.split('.').map(p => parseInt(p, 10) || 0)
  return [parts[0] ?? 0, parts[1] ?? 0, parts[2] ?? 0]
}

function isMajorBump(oldV: string | null, newV: string): boolean {
  if (!oldV) return false
  return parseSemver(oldV)[0] !== parseSemver(newV)[0]
}

// Install phases come from the backend's `InstallProgress` enum; map
// each one to a coarse stepped percentage (the backend doesn't emit
// byte-granular progress, just discrete phases).
function phasePercent(phase?: string | null): number {
  switch (phase) {
    case 'resolving':
      return 10
    case 'downloading':
      return 50
    case 'verifying_sha256':
      return 75
    case 'verifying_cosign':
      return 85
    case 'installing':
      return 95
    case 'complete':
      return 100
    default:
      return 5
  }
}

export function SandboxRootfsVersionsSection() {
  // Memory project_stores_proxy_hooks: every `Stores.X.field` access
  // is `useEffect + useStore` under the hood — read ALL needed fields
  // at the TOP of the component before any early return, else
  // "Rendered more hooks than during the previous render" crashes
  // AppErrorBoundary. `conversationCount` + `mcpServerWorkspaceCount`
  // are consumed inside `handleSetPin`'s major-bump branch but must
  // still be subscribed unconditionally on first paint.
  const {
    pinnedVersion,
    installed,
    available,
    draining,
    lastSwap,
    loading,
    error,
    actions,
    installTasks,
    conversationCount,
    mcpServerWorkspaceCount,
  } = Stores.SandboxRootfsVersions

  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage

  const rows = useMemo(
    () => buildRows(installed, available),
    [installed, available],
  )

  // Map (version, arch, flavor) -> DrainEntry so each row can show
  // its live inflight count + a "Draining" tag when the row's
  // version isn't the current pin but is still mounted.
  const drainByKey = useMemo(() => {
    const m = new Map<string, DrainEntry>()
    for (const d of draining) {
      m.set(`${d.version}::${d.arch}::${d.flavor}`, d)
    }
    return m
  }, [draining])

  // Downloaded-flavors summary for the header card. Distinct
  // (arch, flavor) pairs across every installed artifact row.
  const downloadedFlavors = useMemo(() => {
    const set = new Set<string>()
    for (const a of installed) {
      set.add(`${a.arch}-${a.flavor}`)
    }
    return Array.from(set).sort()
  }, [installed])

  if (!canRead) {
    return (
      <Card title="Rootfs versions">
        <Alert
          type="warning"
          showIcon
          title="You don't have permission to view rootfs versions."
        />
      </Card>
    )
  }

  const handleSetPin = (version: string) => {
    if (isMajorBump(pinnedVersion, version)) {
      const convCount = conversationCount ?? 0
      const mcpCount = mcpServerWorkspaceCount ?? 0
      Modal.confirm({
        title: `Swap to v${version} (major version bump)`,
        content: (
          <div>
            <p>
              The semver major number is changing from v{pinnedVersion} to
              v{version}. To protect against ABI mismatches in Python wheels,
              cargo-installed binaries, and node-native modules baked against
              the old rootfs, the following package-manager subdirs will be
              wiped across{' '}
              <strong>
                {convCount} conversation workspace
                {convCount === 1 ? '' : 's'}
              </strong>{' '}
              and{' '}
              <strong>
                {mcpCount} sandboxed MCP server workspace
                {mcpCount === 1 ? '' : 's'}
              </strong>{' '}
              after in-flight sessions drain:
            </p>
            <ul style={{ marginLeft: 16 }}>
              <li>
                <code>.local</code>, <code>.cache</code>, <code>.npm</code>,{' '}
                <code>.npm-global</code>, <code>.cargo</code>,{' '}
                <code>.rustup</code>, <code>.pyenv</code>,{' '}
                <code>node_modules</code>
              </li>
            </ul>
            <p>
              Your generated files (CSVs, scripts, plots, virtualenvs under
              arbitrary names) are <strong>preserved</strong>. The LLM will
              receive a system note on its next tool call so it knows to
              reinstall any tooling it needs.
            </p>
            <p>
              <strong>
                Running sandboxed MCP servers keep running on the old rootfs
                until they finish; the next request will start them on the
                new one.
              </strong>
            </p>
          </div>
        ),
        okText: 'Swap and wipe caches',
        okButtonProps: { danger: true },
        cancelText: 'Cancel',
        onOk: () => Stores.SandboxRootfsVersions.setPin(version),
        width: 600,
      })
    } else {
      Stores.SandboxRootfsVersions.setPin(version)
    }
  }

  const renderRow = (row: Row) => {
    const key = `${row.version}::${row.arch}::${row.flavor}::${row.pkg}`
    const installState = actions[key]
    const pinState = actions[`pin::${row.version}`]
    const deleteState = row.artifact ? actions[`del::${row.artifact.id}`] : undefined

    const isPinned = pinnedVersion === row.version
    const isInstalled = !!row.artifact
    const drainEntry = drainByKey.get(
      `${row.version}::${row.arch}::${row.flavor}`,
    )
    const live =
      (drainEntry?.inflight_exec ?? 0) + (drainEntry?.inflight_mcp ?? 0)
    const isDraining = !!drainEntry && !isPinned && live > 0
    const task: InstallTaskState | undefined = installTasks[key]
    const isInstalling =
      !!installState?.installing || task?.status === 'running'

    return (
      <div key={key} data-testid={`rootfs-row-${row.version}-${row.flavor}`}>
        <div className="flex items-start gap-3 flex-wrap">
          <div className="flex-1">
            <div className="flex items-center gap-2 mb-2 flex-wrap-reverse">
              <div className="flex-1 min-w-48">
                <Flex align="center" gap="small">
                  <Text className="font-medium">v{row.version}</Text>
                  <Tag>{row.flavor}</Tag>
                  <Tag>{row.arch}</Tag>
                  {isPinned && (
                    <Tag
                      color="blue"
                      icon={<PushpinOutlined />}
                      data-testid="pinned-tag"
                    >
                      Pinned
                    </Tag>
                  )}
                  {isInstalled ? (
                    <Tag
                      icon={<CheckCircleTwoTone twoToneColor="#52c41a" />}
                      color="success"
                    >
                      Downloaded
                    </Tag>
                  ) : (
                    <Tag>Available</Tag>
                  )}
                  {live > 0 && (
                    <Tag
                      color={isDraining ? 'orange' : 'default'}
                      data-testid={`inflight-${row.version}-${row.flavor}`}
                    >
                      {drainEntry?.inflight_exec ?? 0} exec ·{' '}
                      {drainEntry?.inflight_mcp ?? 0} MCP in-flight
                    </Tag>
                  )}
                  {isDraining && (
                    <Tag color="orange" data-testid="row-draining">
                      Draining
                    </Tag>
                  )}
                </Flex>
              </div>
              <div className="flex gap-1 items-center justify-end">
                {!isInstalled && !isInstalling && (
                  <RenderButton
                    canManage={canManage}
                    label="Download"
                    icon={<CloudDownloadOutlined />}
                    loading={false}
                    onClick={() =>
                      Stores.SandboxRootfsVersions.installVersion(
                        row.version,
                        row.arch,
                        row.flavor,
                        row.pkg,
                      )
                    }
                  />
                )}
                {isInstalling && (
                  <div
                    style={{ minWidth: 200 }}
                    data-testid={`install-progress-${row.version}-${row.flavor}`}
                  >
                    <Progress
                      percent={phasePercent(task?.phase)}
                      size="small"
                      status={
                        task?.status === 'failed' ? 'exception' : 'active'
                      }
                    />
                    <div className="text-xs opacity-70">
                      {task?.message ?? task?.phase ?? 'queued'}
                    </div>
                  </div>
                )}
                {isInstalled && !isPinned && (
                  <RenderButton
                    canManage={canManage}
                    label="Pin"
                    icon={<PushpinOutlined />}
                    loading={pinState?.pinning}
                    onClick={() => handleSetPin(row.version)}
                  />
                )}
                {isInstalled && !isPinned && row.artifact && (
                  <Popconfirm
                    title="Delete this artifact?"
                    description="Frees disk; the next execute_command for this flavor at this version re-downloads it. Refused if this version is pinned."
                    okText="Delete"
                    okButtonProps={{ danger: true }}
                    onConfirm={() =>
                      Stores.SandboxRootfsVersions.deleteArtifact(row.artifact!.id)
                    }
                  >
                    <Button
                      danger
                      type="text"
                      icon={<DeleteOutlined />}
                      loading={deleteState?.deleting}
                      disabled={!canManage}
                      data-testid="rootfs-delete-button"
                    >
                      Delete
                    </Button>
                  </Popconfirm>
                )}
              </div>
            </div>
            {row.artifact && (
              <Text type="secondary" className="text-xs block">
                sha256 {row.artifact.sha256.slice(0, 12)}… · downloaded{' '}
                {new Date(row.artifact.downloaded_at).toLocaleDateString()}
                {row.artifact.cosign_bundle ? ' · cosign verified' : ''}
              </Text>
            )}
          </div>
        </div>
      </div>
    )
  }

  return (
    <Card
      title="Rootfs versions"
      extra={
        <Button
          icon={<ReloadOutlined />}
          onClick={() => Stores.SandboxRootfsVersions.loadStatus()}
          data-testid="rootfs-refresh-button"
        >
          Refresh
        </Button>
      }
    >
      <Flex className="flex-col gap-3 mb-3">
        <div>
          <Text strong>Currently pinned: </Text>
          {pinnedVersion ? (
            <Tag color="blue" icon={<PushpinOutlined />} data-testid="pinned-chip">
              v{pinnedVersion}
            </Tag>
          ) : (
            <Tag data-testid="pinned-chip">
              Not yet pinned (will pin on first reachable GitHub call)
            </Tag>
          )}
          {downloadedFlavors.length > 0 && (
            <>
              <Text type="secondary"> · downloaded flavors: </Text>
              <Text data-testid="downloaded-flavors">
                {downloadedFlavors.join(', ')}
              </Text>
            </>
          )}
        </div>
        {lastSwap && lastSwap.draining_mounts > 0 && (
          <Alert
            type="info"
            showIcon
            closable
            title={
              <span data-testid="draining-indicator">
                {lastSwap.draining_mounts} session
                {lastSwap.draining_mounts === 1 ? '' : 's'} still using the
                previous rootfs (v{lastSwap.was}). The old mount will be
                evicted once they finish
                {lastSwap.cache_wipe === 'wipe_caches_on_drain'
                  ? '; the major-bump cache wipe runs after eviction.'
                  : '.'}
              </span>
            }
          />
        )}
      </Flex>
      {error && (
        <Alert type="error" showIcon title={error} className="mb-3" />
      )}
      {loading ? (
        <Spin />
      ) : (
        <Flex className="flex-col gap-4">
          {rows.length === 0 ? (
            <Empty
              description="No rootfs versions yet. GitHub may be unreachable; check `code_sandbox.enabled` in the server config and ensure the server can reach api.github.com."
              image={<CloudDownloadOutlined className="text-4xl opacity-50" />}
            />
          ) : (
            <div>
              {rows.map((row, index) => (
                <div key={`${row.version}-${row.flavor}-${row.arch}-${row.pkg}`}>
                  {renderRow(row)}
                  {index < rows.length - 1 && <Divider className="my-4" />}
                </div>
              ))}
            </div>
          )}
        </Flex>
      )}
    </Card>
  )
}

interface RenderButtonProps {
  canManage: boolean
  label: string
  icon: React.ReactNode
  loading?: boolean
  onClick: () => void
}

function RenderButton({
  canManage,
  label,
  icon,
  loading,
  onClick,
}: RenderButtonProps) {
  const btn = (
    <Button
      type="text"
      icon={icon}
      loading={loading}
      disabled={!canManage || loading}
      onClick={onClick}
    >
      {label}
    </Button>
  )
  return canManage ? btn : <Tooltip title={`Requires ${MANAGE_PERM}`}>{btn}</Tooltip>
}

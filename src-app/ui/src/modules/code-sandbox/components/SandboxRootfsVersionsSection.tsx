import { useMemo } from 'react'
import { Alert, Button, Flex, Modal, Spin, Tag, Typography } from 'antd'
import { ReloadOutlined, StarOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { AvailableRootfsCard } from './AvailableRootfsCard'
import { DownloadedRootfsCard } from './DownloadedRootfsCard'
import {
  buildVersionGroups,
  deriveHostArch,
  isMajorBump,
  MANAGE_PERM,
  READ_PERM,
  type VersionGroup,
} from './_rootfsShared'

const { Text } = Typography

export function SandboxRootfsVersionsSection() {
  // Hook-safety: every `Stores.X.field` read is a `useStore` hook under the
  // hood, so ALL needed fields are read at the TOP before any early return.
  // `conversationCount` / `mcpServerWorkspaceCount` are only consumed inside
  // `handleSetPin`'s major-bump branch but must still subscribe on first paint.
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

  const hostArch = useMemo(() => deriveHostArch(installed), [installed])

  const groups = useMemo(
    () =>
      buildVersionGroups({
        installed,
        available,
        hostArch,
        pinnedVersion,
        installTasks,
        actions,
        draining,
      }),
    [installed, available, hostArch, pinnedVersion, installTasks, actions, draining],
  )

  // Download is atomic over flavors, so a version is normally either fully
  // downloaded (Downloaded card) or not (Available card). A version with only
  // SOME host-arch flavors present (failed/partial download — including the
  // default version) stays in the Available card so its Download button can
  // fetch the missing flavors; RootfsVersionGroup's `!group.isDefault` guard
  // still suppresses Set-default/Delete on the default version there.
  // (`allDownloaded` already implies `anyDownloaded`, so it's the clean
  // complement of the availableGroups filter.)
  const downloadedGroups = useMemo(
    () => groups.filter(g => g.allDownloaded),
    [groups],
  )
  const availableGroups = useMemo(
    () => groups.filter(g => !g.allDownloaded),
    [groups],
  )

  const downloadedFlavors = useMemo(() => {
    const set = new Set<string>()
    for (const a of installed) set.add(`${a.arch}-${a.flavor}`)
    return Array.from(set).sort()
  }, [installed])

  // Secondary per-section gate: a user who reached /settings/sandbox via the
  // resource-limits read permission (the route's anyOf guard) but lacks
  // code_sandbox::environments::read lands here. Mirrors the framework's
  // "Not authorized" wording.
  if (!canRead) {
    return (
      <Alert
        type="warning"
        showIcon
        title="Not authorized"
        description="You don't have permission to view rootfs versions."
      />
    )
  }

  const handleSetPin = (version: string) => {
    if (isMajorBump(pinnedVersion, version)) {
      const convCount = conversationCount ?? 0
      const mcpCount = mcpServerWorkspaceCount ?? 0
      Modal.confirm({
        title: `Set v${version} as default (major version bump)`,
        content: (
          <div>
            <p>
              The semver major number is changing from v{pinnedVersion} to v
              {version}. To protect against ABI mismatches in Python wheels,
              cargo-installed binaries, and node-native modules baked against the
              old rootfs, the following package-manager subdirs will be wiped
              across{' '}
              <strong>
                {convCount} conversation workspace{convCount === 1 ? '' : 's'}
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
                until they finish; the next request will start them on the new
                one.
              </strong>
            </p>
          </div>
        ),
        okText: 'Set as default and wipe caches',
        okButtonProps: { danger: true },
        cancelText: 'Cancel',
        onOk: () => Stores.SandboxRootfsVersions.setPin(version),
        width: 600,
      })
    } else {
      Stores.SandboxRootfsVersions.setPin(version)
    }
  }

  const handleDownloadAll = (group: VersionGroup) => {
    // Download is per-version: fire one install per missing host-arch flavor.
    // Each call seeds its own task; the SSE subscriber drives progress and
    // refreshes the list (migrating the version to the Downloaded card) once
    // every flavor lands.
    for (const f of group.missingFlavors) {
      void Stores.SandboxRootfsVersions.installVersion(
        group.version,
        f.arch,
        f.flavor,
        f.pkg,
      )
    }
  }

  const handleDelete = (group: VersionGroup) => {
    // Version-level delete: remove every downloaded flavor of this version.
    for (const f of group.flavors) {
      if (f.artifact) {
        void Stores.SandboxRootfsVersions.deleteArtifact(f.artifact.id)
      }
    }
  }

  return (
    <Flex vertical className="gap-3">
      <Flex align="center" justify="space-between" wrap className="gap-2">
        <div>
          <Text strong>Currently default: </Text>
          {pinnedVersion ? (
            <Tag color="blue" icon={<StarOutlined />} data-testid="default-chip">
              v{pinnedVersion}
            </Tag>
          ) : (
            <Tag data-testid="default-chip">
              Not yet set (defaults on first reachable GitHub call)
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
        <Button
          icon={<ReloadOutlined />}
          onClick={() => Stores.SandboxRootfsVersions.loadStatus()}
          data-testid="rootfs-refresh-button"
        >
          Refresh
        </Button>
      </Flex>

      {lastSwap && lastSwap.draining_mounts > 0 && (
        <Alert
          type="info"
          showIcon
          closable
          title={
            <span data-testid="draining-indicator">
              {lastSwap.draining_mounts} session
              {lastSwap.draining_mounts === 1 ? '' : 's'} still using the
              previous rootfs (v{lastSwap.was}). The old mount will be evicted
              once they finish
              {lastSwap.cache_wipe === 'wipe_caches_on_drain'
                ? '; the major-bump cache wipe runs after eviction.'
                : '.'}
            </span>
          }
        />
      )}

      {error && <Alert type="error" showIcon title={error} />}

      {loading && groups.length === 0 ? (
        <Spin />
      ) : (
        <>
          <DownloadedRootfsCard
            groups={downloadedGroups}
            canManage={canManage}
            actions={actions}
            onSetDefault={handleSetPin}
            onDelete={handleDelete}
          />
          <AvailableRootfsCard
            groups={availableGroups}
            canManage={canManage}
            onDownloadAll={handleDownloadAll}
          />
        </>
      )}
    </Flex>
  )
}

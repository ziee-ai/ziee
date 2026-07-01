import { useMemo } from 'react'
import { Alert, Button, Flex, Spin, Tag, Text, message } from '@/components/ui'
import { RotateCw, Star } from 'lucide-react'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { AvailableRootfsCard } from './AvailableRootfsCard'
import { DownloadedRootfsCard } from './DownloadedRootfsCard'
import {
  buildVersionGroups,
  deriveHostArch,
  deriveHostPackage,
  isMajorBump,
  MANAGE_PERM,
  READ_PERM,
  type VersionGroup,
} from './_rootfsShared'

export function SandboxRootfsVersionsSection() {
  const { dialog } = require('@/components/ui')
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
    hostArch: serverHostArch,
    hostPackage: serverHostPackage,
  } = Stores.SandboxRootfsVersions

  const canManage = usePermission(MANAGE_PERM)
  const canRead = usePermission(READ_PERM) || canManage

  // Prefer the server-authoritative host arch/package (correct even on a fresh,
  // zero-installed host — e.g. Windows/WSL2 needs tar.zst); fall back to the
  // installed-artifact heuristic if the server didn't supply them.
  const hostArch = useMemo(
    () => serverHostArch ?? deriveHostArch(installed),
    [serverHostArch, installed],
  )
  const hostPkg = useMemo(
    () => serverHostPackage ?? deriveHostPackage(installed),
    [serverHostPackage, installed],
  )

  const groups = useMemo(
    () =>
      buildVersionGroups({
        installed,
        available,
        hostArch,
        hostPkg,
        pinnedVersion,
        installTasks,
        actions,
        draining,
      }),
    [installed, available, hostArch, hostPkg, pinnedVersion, installTasks, actions, draining],
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
        tone="warning"
        title="Not authorized"
        description="You don't have permission to view rootfs versions."
        data-testid="sandbox-rootfs-noperm-alert"
      />
    )
  }

  const doSetPin = async (version: string) => {
    const ok = await Stores.SandboxRootfsVersions.setPin(version)
    if (ok) message.success(`Default rootfs set to v${version}`)
    else message.error(`Failed to set default rootfs to v${version}`)
  }

  const handleSetPin = (version: string) => {
    if (isMajorBump(pinnedVersion, version)) {
      const convCount = conversationCount ?? 0
      const mcpCount = mcpServerWorkspaceCount ?? 0
      void dialog.confirm({
        title: `Set v${version} as default (major version bump)`,
        description: (
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
        cancelText: 'Cancel',
        testid: 'sandbox-major-bump-confirm',
      }).then(ok => {
        // `dialog.confirm` resolves a boolean (there is no `onConfirm`
        // callback option — it was silently ignored, so confirming did
        // nothing). Run the pin only when the admin confirms.
        if (ok) void doSetPin(version)
      })
    } else {
      void doSetPin(version)
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

  const handleDelete = async (group: VersionGroup) => {
    // Version-level delete: remove every downloaded flavor of this version.
    const ids = group.flavors
      .map(f => f.artifact?.id)
      .filter((id): id is string => !!id)
    const results = await Promise.all(
      ids.map(id => Stores.SandboxRootfsVersions.deleteArtifact(id)),
    )
    const failed = results.filter(ok => !ok).length
    if (failed === 0) {
      message.success(`Deleted rootfs v${group.version}`)
    } else {
      message.error(
        `Failed to delete ${failed} of ${results.length} flavor(s) of v${group.version}`,
      )
    }
  }

  return (
    <Flex vertical className="gap-3">
      <Flex align="center" justify="between" wrap className="gap-2">
        <div>
          <Text strong>Currently default: </Text>
          {pinnedVersion ? (
            <Tag tone="info" icon={<Star />} data-testid="default-chip">
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
          icon={<RotateCw />}
          onClick={() => Stores.SandboxRootfsVersions.loadStatus({ pruneFailed: true })}
          data-testid="rootfs-refresh-button"
        >
          Refresh
        </Button>
      </Flex>

      {lastSwap && lastSwap.draining_mounts > 0 && (
        <Alert
          tone="info"
          onClose={() => {}}
          closeLabel="Close"
          data-testid="sandbox-rootfs-draining-alert"
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

      {error && <Alert tone="error" title={error} data-testid="sandbox-rootfs-error-alert" />}

      {loading && groups.length === 0 ? (
        <Spin label="Loading rootfs versions" />
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

import { Button, Flex, Confirm, Progress, Tag, Tooltip, Text } from '@/components/ui'
import { CircleCheck, CloudDownload, Trash2, Star } from 'lucide-react'
import {
  MANAGE_PERM,
  phasePercent,
  RenderButton,
  type FlavorEntry,
  type VersionGroup,
} from './_rootfsShared'

interface RootfsVersionGroupProps {
  group: VersionGroup
  /** "available" → Download-all primary action; "downloaded" → Set-default + Delete. */
  variant: 'available' | 'downloaded'
  canManage: boolean
  // Each card only owns the handlers for its variant, so the others are optional.
  onDownloadAll?: (group: VersionGroup) => void
  onSetDefault?: (version: string) => void
  onDelete?: (group: VersionGroup) => void
  setDefaultLoading?: boolean
  deleteLoading?: boolean
}

/**
 * One version block: a header row (version label + status tags + the
 * right-aligned primary action) followed by the nested per-flavor sub-rows and
 * — while a download is in flight — a single version-level aggregate progress
 * bar derived from the per-flavor install tasks.
 */
export function RootfsVersionGroup({
  group,
  variant,
  canManage,
  onDownloadAll,
  onSetDefault,
  onDelete,
  setDefaultLoading,
  deleteLoading,
}: RootfsVersionGroupProps) {
  const installing = group.flavors.filter(f => f.isInstalling)
  const failed = group.flavors.filter(f => f.task?.status === 'failed')
  const showProgress = installing.length > 0 || failed.length > 0
  // Monotonic aggregate: average over ALL of the version's flavors, counting an
  // already-downloaded flavor as 100. As each flavor finishes (its artifact
  // appears via loadStatus, its task is pruned), its contribution rises to 100,
  // so the bar can't regress when one flavor completes before another.
  const aggPercent = showProgress
    ? Math.round(
        group.flavors.reduce(
          (sum, f) => sum + (f.artifact ? 100 : phasePercent(f.task?.phase)),
          0,
        ) / group.flavors.length,
      )
    : 100
  const progressMessage =
    failed.length > 0
      ? (failed[0].task?.error ?? 'Install failed')
      : installing.length > 0
        ? `Installing ${installing.map(f => f.flavor).join(', ')}… (${
            installing[0].task?.message ?? installing[0].task?.phase ?? 'queued'
          })`
        : ''

  const anyDraining = group.flavors.some(f => f.isDraining)

  return (
    <Flex
      vertical
      gap="small"
      data-testid={`rootfs-version-group-${group.version}`}
    >
      <Flex align="center" gap="small" justify="between" wrap>
        <Flex align="center" gap="small" wrap className="min-w-48">
          <Text className="font-medium">v{group.version}</Text>
          {/* A filled blue Tag (not the sibling module's muted "(Default)"
              text) to match THIS page's header "Currently default" chip. */}
          {group.isDefault && (
            <Tag variant="outline" tone="info" icon={<Star />} data-testid="default-tag">
              Default
            </Tag>
          )}
          {anyDraining && (
            <Tag variant="outline" tone="warning" data-testid="row-draining">
              Draining
            </Tag>
          )}
        </Flex>

        <Flex align="center" gap="small" justify="end">
          {variant === 'available' && (
            <RenderButton
              canManage={canManage}
              label={installing.length > 0 ? 'Installing…' : 'Download'}
              icon={<CloudDownload />}
              loading={installing.length > 0}
              onClick={() => onDownloadAll?.(group)}
              data-testid={`rootfs-download-${group.version}`}
            />
          )}
          {variant === 'downloaded' && !group.isDefault && (
            <>
              <RenderButton
                canManage={canManage}
                label="Set as Default"
                icon={<Star />}
                loading={setDefaultLoading}
                onClick={() => onSetDefault?.(group.version)}
                data-testid={`rootfs-set-default-${group.version}`}
              />
              <DeleteVersionButton
                canManage={canManage}
                loading={deleteLoading}
                version={group.version}
                onConfirm={() => onDelete?.(group)}
              />
            </>
          )}
        </Flex>
      </Flex>

      {showProgress && (
        <div data-testid={`install-progress-${group.version}`}>
          <Progress value={aggPercent} size="sm" tone={failed.length > 0 ? 'error' : 'primary'} aria-label="Install progress" data-testid={`sandbox-install-progress-bar-${group.version}`} />
          {progressMessage && (
            <Text type="secondary" className="text-xs">
              {progressMessage}
            </Text>
          )}
        </div>
      )}

      <Flex vertical gap="small" className="pl-1">
        {group.flavors.map(f => (
          <FlavorSubRow key={f.rowKey} version={group.version} flavor={f} />
        ))}
      </Flex>
    </Flex>
  )
}

/**
 * Version-level Delete with the same visible-but-disabled + "Requires …manage"
 * Tooltip affordance the RenderButton-based actions use. The Button must remain
 * the single trigger child of Confirm, so the Tooltip wraps the whole
 * Confirm; `disabled` keeps the Confirm from opening for read-only users.
 */
function DeleteVersionButton({
  canManage,
  loading,
  version,
  onConfirm,
}: {
  canManage: boolean
  loading?: boolean
  version: string
  onConfirm: () => void
}) {
  const del = (
    <Confirm
      data-testid={`sandbox-delete-confirm-${version}`}
      title="Delete this version?"
      description="Removes all downloaded flavors of this version. Frees disk; the next use re-downloads. Refused while it is the default."
      okText="OK"
      cancelText="Cancel"
      okButtonProps={{ danger: true }}
      onConfirm={onConfirm}
    >
      <Button
        variant="ghost"
        icon={<Trash2 />}
        loading={loading}
        disabled={!canManage}
        data-testid={`rootfs-delete-${version}`}
      >
        Delete
      </Button>
    </Confirm>
  )
  return canManage ? (
    del
  ) : (
    <Tooltip content={`Requires ${MANAGE_PERM}`}>{del}</Tooltip>
  )
}

function FlavorSubRow({
  version,
  flavor: f,
}: {
  version: string
  flavor: FlavorEntry
}) {
  return (
    <div data-testid={`rootfs-row-${version}-${f.flavor}`}>
      <Flex align="center" gap="small" wrap>
        <Tag variant="outline" data-testid={`sandbox-flavor-tag-${version}-${f.flavor}`}>{f.flavor}</Tag>
        <Tag variant="outline" data-testid={`sandbox-arch-tag-${version}-${f.flavor}`}>{f.arch}</Tag>
        {f.artifact ? (
          <Tag variant="outline"
            icon={<CircleCheck />}
            tone="success"
            data-testid={`sandbox-status-tag-${version}-${f.flavor}`}
            data-state="downloaded"
          >
            Downloaded
          </Tag>
        ) : (
          <Tag variant="outline"
            data-testid={`sandbox-status-tag-${version}-${f.flavor}`}
            data-state="available"
          >
            Available
          </Tag>
        )}
        {f.live > 0 && (
          <Tag variant="outline"
            tone={f.isDraining ? 'warning' : undefined}
            data-testid={`inflight-${version}-${f.flavor}`}
          >
            {f.drainEntry?.inflight_exec ?? 0} exec ·{' '}
            {f.drainEntry?.inflight_mcp ?? 0} MCP in-flight
          </Tag>
        )}
      </Flex>
      {f.artifact && (
        <Text type="secondary" className="text-xs block">
          sha256 {f.artifact.sha256.slice(0, 12)}… · downloaded{' '}
          {new Date(f.artifact.downloaded_at).toLocaleDateString()}
          {f.artifact.cosign_bundle ? ' · cosign verified' : ''}
        </Text>
      )}
    </div>
  )
}

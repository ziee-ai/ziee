import { RotateCw } from 'lucide-react'
import { Fragment, useEffect } from 'react'
import { Button, Card, Separator, Empty, Flex, Spin, Tag, Text } from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import type { RuntimeEngine } from '../types'
import { RuntimeVersionCard } from './RuntimeVersionCard'
import { VersionModelsBlock } from './VersionModelsBlock'

/**
 * Per-engine "Installed versions" card.
 *
 * Each row stacks:
 *   - the version header (RuntimeVersionCard — title + actions on
 *     the top-right, compact metadata strip below), then
 *   - the models that effectively run on that version, inlined
 *     directly underneath (VersionModelsBlock — provider-grouped,
 *     start/stop/restart/swap + Logs disclosure).
 *
 * The dedicated "Models by engine version" card has been folded
 * into these rows so an operator looking at "which versions am I
 * running?" sees, in one place, "v0.0.1 — these 3 models pin it,
 * 1 is running."
 *
 * A footer block lists local models for this engine whose pinned
 * version isn't installed (was the unresolved-warning section in
 * the old standalone card).
 *
 * Loads both `Stores.RuntimeVersion.versions` (for the version
 * rows) and `Stores.RuntimeModelUsage.usage` (for the per-version
 * model lists + the unresolved set).
 */
export function InstalledVersionsCard({ engine }: { engine: RuntimeEngine }) {
  const { versions, loading: loadingVersions } = Stores.RuntimeVersion
  const { usage, loading: loadingUsage } = Stores.RuntimeModelUsage
  const canManage = usePermission(Permissions.LocalRuntimeManage)
  const canViewLogs = usePermission(Permissions.LocalRuntimeLogs)
  const refreshing = loadingVersions || (loadingUsage.get(engine) ?? false)

  const handleRefresh = () => {
    // Reload BOTH stores — the version list (Stores.RuntimeVersion)
    // and the per-version models (Stores.RuntimeModelUsage).
    // Fire-and-forget; the loading spinners on the button + spinner
    // for empty state cover the user-visible state.
    Stores.RuntimeVersion.loadVersions().catch(() => {})
    Stores.RuntimeModelUsage.loadUsage(engine).catch(() => {})
  }

  const engineVersions = versions.filter(v => v.engine === engine)
  const engineUsage = usage.get(engine)

  // Swap-dropdown options live on the parent: pre-built once from
  // engineUsage so every child row's `<Select>` agrees on the
  // available set + their labels.
  const versionOptions = (engineUsage?.versions ?? []).map(v => ({
    value: v.version.id,
    label: v.version.is_system_default
      ? `${v.version.version} (${v.version.backend}, default)`
      : `${v.version.version} (${v.version.backend})`,
  }))

  // Quick lookup: version_id → models that resolve to it.
  const modelsByVersion = new Map(
    (engineUsage?.versions ?? []).map(v => [v.version.id, v.models]),
  )

  useEffect(() => {
    if (versions.length === 0 && !loadingVersions) {
      Stores.RuntimeVersion.loadVersions().catch(() => {})
    }
    // Models-by-version is a separate fetch — it returns "models
    // for installed versions" which is data Stores.RuntimeVersion
    // doesn't carry. Loaded every mount so a swap/start/stop in
    // another tab is reflected the next time the card paints.
    Stores.RuntimeModelUsage.loadUsage(engine).catch(() => {})
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [engine])

  return (
    <Card
      title="Installed versions"
      data-testid={`llmrt-installed-versions-card-${engine}`}
      extra={
        <Button
          icon={<RotateCw />}
          loading={refreshing}
          onClick={handleRefresh}
          data-testid={`llmrt-installed-refresh-${engine}`}
          aria-label={`Refresh installed ${engine} versions`}
        >
          Refresh
        </Button>
      }
    >
      {loadingVersions && engineVersions.length === 0 ? (
        <Spin label="Loading" />
      ) : engineVersions.length === 0 ? (
        <Empty
          description="No versions installed yet — install one below."
          data-testid={`llmrt-installed-empty-${engine}`}
        />
      ) : (
        <div>
          {engineVersions.map((v, i) => (
            <Fragment key={v.id}>
              {i > 0 && <Separator className="!my-4" />}
              {/* Installed rows are static (not "selectable"), so no
                  hover background — matches the User / Repository
                  settings pages, which also don't tint rows on hover. */}
              <Flex vertical gap="small">
                <RuntimeVersionCard version={v} />
                <VersionModelsBlock
                  engine={engine}
                  versionId={v.id}
                  models={modelsByVersion.get(v.id) ?? []}
                  versionOptions={versionOptions}
                  canManage={canManage}
                  canViewLogs={canViewLogs}
                />
              </Flex>
            </Fragment>
          ))}
          {engineUsage?.unresolved && engineUsage.unresolved.length > 0 && (
            <>
              <Separator className="!my-4" />
              <Flex vertical gap="small">
                <Text type="warning">
                  No installed version resolves for these models:
                </Text>
                <div>
                  {engineUsage.unresolved.map(m => (
                    <Tag key={m.id} data-testid={`llmrt-unresolved-tag-${m.id}`}>{m.display_name}</Tag>
                  ))}
                </div>
              </Flex>
            </>
          )}
        </div>
      )}
    </Card>
  )
}

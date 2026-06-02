import { Card, Divider, Empty, Flex } from 'antd'
import { Fragment } from 'react'
import { RootfsVersionGroup } from './RootfsVersionGroup'
import type { ActionFlags, VersionGroup } from './_rootfsShared'

interface DownloadedRootfsCardProps {
  groups: VersionGroup[]
  canManage: boolean
  actions: Record<string, ActionFlags>
  onSetDefault: (version: string) => void
  onDelete: (group: VersionGroup) => void
}

/** "Downloaded versions" — fully-downloaded versions with version-level
 *  Set-as-default + Delete actions; flavors render as informational sub-rows. */
export function DownloadedRootfsCard({
  groups,
  canManage,
  actions,
  onSetDefault,
  onDelete,
}: DownloadedRootfsCardProps) {
  return (
    <Card title="Downloaded versions" data-testid="downloaded-versions-card">
      {groups.length === 0 ? (
        <Empty
          image={Empty.PRESENTED_IMAGE_SIMPLE}
          description="No rootfs versions downloaded yet. Download one from the Available versions list below."
        />
      ) : (
        <Flex vertical className="gap-1">
          {groups.map((g, i) => (
            <Fragment key={g.version}>
              {i > 0 && <Divider className="!my-3" />}
              <RootfsVersionGroup
                group={g}
                variant="downloaded"
                canManage={canManage}
                onSetDefault={onSetDefault}
                onDelete={onDelete}
                setDefaultLoading={actions[`pin::${g.version}`]?.pinning}
                deleteLoading={g.flavors.some(
                  f => f.artifact && actions[`del::${f.artifact.id}`]?.deleting,
                )}
              />
            </Fragment>
          ))}
        </Flex>
      )}
    </Card>
  )
}

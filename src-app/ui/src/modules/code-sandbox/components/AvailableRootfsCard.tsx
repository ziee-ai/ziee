import { Card, Empty, Flex, Separator } from '@/components/ui'
import { Fragment } from 'react'
import { RootfsVersionGroup } from './RootfsVersionGroup'
import type { VersionGroup } from './_rootfsShared'

interface AvailableRootfsCardProps {
  groups: VersionGroup[]
  canManage: boolean
  onDownloadAll: (group: VersionGroup) => void
}

/** "Available versions" — GitHub-catalog versions not yet fully downloaded.
 *  A single per-version Download button fetches every missing host-arch flavor. */
export function AvailableRootfsCard({
  groups,
  canManage,
  onDownloadAll,
}: AvailableRootfsCardProps) {
  return (
    <Card title="Available versions" data-testid="available-versions-card">
      {groups.length === 0 ? (
        <Empty
          description="No versions available to download. GitHub Releases may be unreachable, or no compatible releases were found — ensure the server can reach api.github.com, then Refresh."
          data-testid="sandbox-available-empty"
        />
      ) : (
        <Flex vertical className="gap-1">
          {groups.map((g, i) => (
            <Fragment key={g.version}>
              {i > 0 && <Separator className="!my-3" />}
              <RootfsVersionGroup
                group={g}
                variant="available"
                canManage={canManage}
                onDownloadAll={onDownloadAll}
              />
            </Fragment>
          ))}
        </Flex>
      )}
    </Card>
  )
}

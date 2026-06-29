/**
 * App-wide "update available" banner (web UI, admin-only).
 *
 * Rendered at the top of the content area in AppLayout. Shows only when the
 * server's daily check found a newer version and the admin hasn't dismissed it
 * this session. Gated by <Can> so only admins ever see/mount it.
 */

import { Alert, Button, Link } from '@/components/ui'
import { useNavigate } from 'react-router-dom'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { Stores } from '@/core/stores'

export function ServerUpdateBanner() {
  return (
    <Can permission={Permissions.ServerUpdateRead}>
      <ServerUpdateBannerInner />
    </Can>
  )
}

function ServerUpdateBannerInner() {
  const navigate = useNavigate()
  const { updateAvailable, dismissed, latestVersion, releaseUrl } = Stores.ServerUpdate

  if (!updateAvailable || dismissed) return null

  return (
    <Alert
      data-testid="serverupd-banner-alert"
      tone="info"
      onClose={() => Stores.ServerUpdate.dismiss()}
      closeLabel="Close"
      title={
        <span>
          Ziee {latestVersion} is available.{' '}
          <Button
            data-testid="serverupd-banner-howto-btn"
            variant="link"
            size="sm"
            className="!p-0 !h-auto"
            onClick={() => navigate('/settings/about')}
          >
            How to update
          </Button>
          {releaseUrl && (
            <>
              {' · '}
              <Link href={releaseUrl} target="_blank" rel="noreferrer">
                Release notes
              </Link>
            </>
          )}
        </span>
      }
    />
  )
}

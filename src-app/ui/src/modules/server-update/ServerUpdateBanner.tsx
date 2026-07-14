/**
 * App-wide "update available" banner (web UI, admin-only).
 *
 * Rendered at the top of the content area in AppLayout. Shows only when the
 * server's daily check found a newer version and the admin hasn't dismissed it
 * this session. Gated by <Can> so only admins ever see/mount it.
 */

import { Alert, Button, Link } from '@ziee/kit'
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
  const { enabled, updateAvailable, dismissed, latestVersion, releaseUrl } =
    Stores.ServerUpdate

  // Never surface an update prompt when update checks are disabled in server
  // config (air-gapped deployments) — guard against a stale `updateAvailable`
  // outliving an `enabled: false` flip.
  if (!enabled || !updateAvailable || dismissed) return null

  return (
    <Alert
      data-testid="serverupd-banner-alert"
      tone="info"
      aria-label="Software update available"
      onClose={() => Stores.ServerUpdate.dismiss()}
      closeLabel="Close"
      title={
        <span>
          Ziee {latestVersion} is available.{' '}
          <Button
            data-testid="serverupd-banner-howto-btn"
            variant="link"
            size="default"
            className="!p-0 !h-auto"
            onClick={() => navigate('/settings/about')}
          >
            How to update
          </Button>
          {releaseUrl && (
            <>
              {' · '}
              <Link data-testid="serverupd-banner-release-notes-link" href={releaseUrl} target="_blank" rel="noreferrer">
                Release notes
              </Link>
            </>
          )}
        </span>
      }
    />
  )
}

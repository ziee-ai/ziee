/**
 * App-wide "update available" banner (web UI, admin-only).
 *
 * Rendered at the top of the content area in AppLayout. Shows only when the
 * server's daily check found a newer version and the admin hasn't dismissed it
 * this session. Gated by <Can> so only admins ever see/mount it.
 */

import { Alert, Button, Typography } from 'antd'
import { useNavigate } from 'react-router-dom'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { Stores } from '@/core/stores'

const { Link } = Typography

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
      type="info"
      showIcon
      banner
      role="status"
      aria-label="Software update available"
      // antd v6 only renders the close control for the object form when
      // `closeIcon` is truthy (Alert.js isClosable: `isPlainObject(closable) &&
      // closable.closeIcon`); `closeIcon: true` → default CloseOutlined.
      closable={{ closeIcon: true, onClose: () => Stores.ServerUpdate.dismiss() }}
      title={
        <span>
          Ziee {latestVersion} is available.{' '}
          <Button
            type="link"
            size="small"
            style={{ padding: 0, height: 'auto' }}
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

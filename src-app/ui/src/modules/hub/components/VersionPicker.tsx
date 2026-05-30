import { useState } from 'react'
import { Button, Dropdown, Tag, Tooltip, message } from 'antd'
import { DownOutlined, CheckOutlined } from '@ant-design/icons'
import type { MenuProps } from 'antd'
import { Stores } from '@/core/stores'

const TRACK_LATEST_KEY = '__latest__'

/**
 * Admin catalog-version picker shown in the HubPage header. Lists the
 * versions published on ziee-ai/hub and lets an admin activate one
 * server-wide (or "Track latest" to clear the pin). Non-admins never
 * see this — HubPage renders a read-only Tag instead.
 */
export function VersionPicker() {
  const releases = Stores.HubCatalog.releases
  const activeVersion = Stores.HubCatalog.activeVersion
  const pinnedVersion = Stores.HubCatalog.pinnedVersion
  const hubVersion = Stores.HubCatalog.hubVersion
  const releasesLoading = Stores.HubCatalog.releasesLoading
  const activating = Stores.HubCatalog.activating
  const [open, setOpen] = useState(false)

  const handleOpenChange = (next: boolean) => {
    setOpen(next)
    if (next) {
      // Lazy-load the version list the first time the dropdown opens.
      Stores.HubCatalog.loadReleases()
    }
  }

  const activate = async (version: string | null) => {
    try {
      await Stores.HubCatalog.activateVersion(version)
      message.success(
        version
          ? `Activated hub catalog v${version}`
          : 'Now tracking the latest hub catalog',
      )
    } catch (e) {
      message.error(
        `Failed to activate version: ${(e as Error)?.message ?? e}`,
      )
    }
    setOpen(false)
  }

  const items: MenuProps['items'] = [
    {
      key: TRACK_LATEST_KEY,
      label: (
        <span>
          Track latest{' '}
          {pinnedVersion === null && <CheckOutlined className="ml-1" />}
        </span>
      ),
    },
    { type: 'divider' },
    ...(releasesLoading
      ? [{ key: '__loading__', label: 'Loading versions…', disabled: true }]
      : releases.length === 0
        ? [{ key: '__empty__', label: 'No versions available', disabled: true }]
        : releases.map(r => ({
            key: r.version,
            label: (
              <span className="flex items-center gap-2">
                v{r.version}
                {r.prerelease && (
                  <Tag color="orange" className="!m-0">
                    pre
                  </Tag>
                )}
                {r.version === activeVersion && (
                  <Tooltip title="Currently active">
                    <CheckOutlined />
                  </Tooltip>
                )}
                {r.version === pinnedVersion && (
                  <Tag className="!m-0">pinned</Tag>
                )}
              </span>
            ),
          }))),
  ]

  const onClick: MenuProps['onClick'] = ({ key }) => {
    if (key === TRACK_LATEST_KEY) {
      void activate(null)
    } else if (!key.startsWith('__')) {
      void activate(key)
    }
  }

  return (
    <Dropdown
      open={open}
      onOpenChange={handleOpenChange}
      menu={{ items, onClick, selectable: false }}
      trigger={['click']}
    >
      <Button
        type="text"
        size="small"
        loading={activating}
        aria-label="Select hub catalog version"
        data-testid="hub-version-picker"
      >
        <Tag className="!m-0">v{hubVersion ?? '…'}</Tag>
        <DownOutlined />
      </Button>
    </Dropdown>
  )
}

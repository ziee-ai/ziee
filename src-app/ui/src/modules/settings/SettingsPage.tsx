import { Button, Dropdown, Flex, Menu, Result, theme, Typography } from 'antd'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useElementMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { IoIosArrowDown, IoMdSettings } from 'react-icons/io'
import { useEffect, useRef } from 'react'
import { Stores } from '@/core/stores'
import { evaluatePermission } from '@/core/permissions'
import type { SettingsPageSlot } from '@/modules/settings/types/SettingsSlots'

export default function SettingsPage() {
  const navigate = useNavigate()
  const location = useLocation()
  // Drive layout from the settings page's OWN container width
  // (via ResizeObserver on `containerRef`). Independent of the
  // viewport AND of `mainContentWidth` — guarantees the layout
  // flip is keyed to the actual horizontal room the settings
  // page has, regardless of what's happening upstream in the
  // AppLayout (sidebar collapse, window resize, embedded chrome).
  const containerRef = useRef<HTMLDivElement>(null)
  const minSize = useElementMinSize(containerRef)
  // The settings layout needs the side-menu (~180px) + a content
  // column wide enough for cards/forms (~440px) to feel non-cramped
  // — that's ~620px total. Use `sm` (≤640px) as the threshold so
  // the menu folds into the mobile dropdown the moment the page
  // itself drops below the comfortable two-column width, not at
  // the much tighter `xs` (≤480) which the page rarely hits.
  const useMobileLayout = minSize.sm
  const { token } = theme.useToken()

  const { slots } = Stores.ModuleSystem
  const { user, permissions } = Stores.Auth

  const isAllowed = (item: SettingsPageSlot) =>
    !item.permission || evaluatePermission(user, permissions, item.permission)

  // Get, sort, and permission-filter user settings from slots
  const userSettingsItems = (slots.get('settingsUserPages') || [])
    .filter(isAllowed)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))

  // Get, sort, and permission-filter admin settings from slots
  const adminSettingsItems = (slots.get('settingsAdminPages') || [])
    .filter(isAllowed)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))

  // Slot entries the user can't see, kept around so we can distinguish
  // "section doesn't exist" from "section exists but you're forbidden"
  // when handling deep-link URLs below.
  const forbiddenSettingsItems = [
    ...(slots.get('settingsUserPages') || []),
    ...(slots.get('settingsAdminPages') || []),
  ].filter(item => !isAllowed(item))

  // Build final menu
  const menuItems = [
    ...userSettingsItems.map(item => ({
      key: item.path,
      icon: item.icon,
      label: item.label,
    })),
    ...(adminSettingsItems.length > 0
      ? [
          { type: 'divider' as const },
          {
            key: 'admin',
            icon: <IoMdSettings />,
            label: 'Admin',
            type: 'group' as const,
          },
          ...adminSettingsItems.map(item => ({
            key: item.path,
            icon: item.icon,
            label: item.label,
          })),
        ]
      : []),
  ]

  // Extract the current settings section from the URL and validate it
  const urlSection = location.pathname.match(/\/settings\/([^/]+)/)?.[1]
  const validSections = menuItems
    .filter(
      item =>
        'key' in item &&
        item.key &&
        (item as any).type !== 'divider' &&
        (item as any).type !== 'group',
    )
    .map(item => (item as any).key)

  // Did the user deep-link to a section that exists but their
  // permissions hide? Treat that distinctly from "section doesn't
  // exist" — render an inline 403 rather than silently redirecting,
  // so admin-shared links produce a meaningful page.
  const forbiddenSection = urlSection
    ? forbiddenSettingsItems.find(item => item.path === urlSection)
    : undefined

  const currentSection = validSections.includes(urlSection)
    ? urlSection
    : validSections[0]

  // Redirect to first available settings page if at root /settings.
  // Skip redirect when the URL points at a forbidden section — we want
  // to render the 403 panel in place, not bounce the user elsewhere.
  useEffect(() => {
    if (
      (location.pathname === '/settings' ||
        location.pathname === '/settings/') &&
      !forbiddenSection
    ) {
      if (validSections.length > 0) {
        navigate(`/settings/${validSections[0]}`, { replace: true })
      }
    }
  }, [location.pathname, navigate, validSections, forbiddenSection])

  const handleMenuClick = (key: string) => {
    navigate(`/settings/${key}`)
  }

  // Get current section display info
  const getCurrentSectionInfo = () => {
    const currentItem = menuItems.find(
      item => 'key' in item && item.key === currentSection,
    )
    return currentItem || { icon: <IoMdSettings />, label: 'Settings' }
  }

  const SettingsMenu = () => (
    <Menu
      className={`
      w-fit
      h-full
      !p-1
      !border-r-0
      [&_.ant-menu]:!px-2
      [&_.ant-menu-item]:!h-8
      [&_.ant-menu-item]:!leading-[32px]
      `}
      style={{
        lineHeight: 1,
      }}
      selectedKeys={[currentSection || validSections[0]]}
      items={menuItems}
      onClick={({ key }) => handleMenuClick(key)}
    />
  )

  return (
    <div
      ref={containerRef}
      className="h-full flex flex-col overflow-hidden"
    >
      {/* Page Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Typography.Title level={4} className="!m-0 !leading-tight truncate">
            Settings
          </Typography.Title>
          {useMobileLayout && (
            <div className="flex flex-1 items-center px-2">
              <Dropdown
                styles={{
                  root: {
                    border: '1px solid ' + token.colorBorderSecondary,
                  },
                }}
                classNames={{
                  root: `
                  rounded-md
                  `,
                }}
                menu={{
                  items: menuItems.map((item: any) => {
                    if ('type' in item && item.type === 'divider') {
                      return { type: 'divider' }
                    }
                    if ('type' in item && item.type === 'group') {
                      return {
                        type: 'group',
                        label: (
                          <div className={'-ml-1'}>
                            <Typography.Text
                              strong
                              type={'secondary'}
                              className={'!text-xs'}
                            >
                              {item.label}
                            </Typography.Text>
                          </div>
                        ),
                      }
                    }
                    return {
                      key: item.key,
                      label: (
                        <Flex className={'gap-2 items-center'}>
                          {item.icon}
                          {item.label}
                        </Flex>
                      ),
                    }
                  }),
                  onClick: ({ key }) => {
                    handleMenuClick(key)
                  },
                  selectedKeys: [currentSection || validSections[0]],
                }}
                trigger={['click']}
              >
                <Button
                  type="text"
                  className={'mt-[2px]'}
                  aria-label="Select settings section"
                  aria-haspopup="menu"
                >
                  {getCurrentSectionInfo().icon} {getCurrentSectionInfo().label}{' '}
                  <IoIosArrowDown />
                </Button>
              </Dropdown>
            </div>
          )}
        </div>
      </HeaderBarContainer>

      {/* Page Content */}
      <div className="flex flex-1 overflow-hidden">
        {/* Desktop Sidebar — top padding gives the menu a 16px gap
            from the HeaderBarContainer above. Without it the menu's
            first item sits flush against the bottom of the header,
            which fights the soft fade overlay HeaderBarContainer
            paints below itself. */}
        {!useMobileLayout && (
          <div className="w-fit pt-1">
            <SettingsMenu />
          </div>
        )}

        {/* Main Content Area */}
        <div className="flex-1 overflow-hidden">
          {forbiddenSection ? (
            <Result
              status="403"
              title="Not authorized"
              subTitle={`You don't have permission to view "${forbiddenSection.label}".`}
            />
          ) : (
            <Outlet />
          )}
        </div>
      </div>
    </div>
  )
}

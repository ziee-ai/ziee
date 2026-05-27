/**
 * Desktop Override: SettingsPage
 *
 * Filters out admin settings that are not relevant for desktop app:
 * - Users
 * - User Groups
 * - Assistants
 */

import { Button, Dropdown, Flex, Menu, theme, Typography } from 'antd'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { IoIosArrowDown, IoMdSettings } from 'react-icons/io'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'

// Admin settings to hide in desktop app (single-admin, no external auth).
// `auth-providers` is LDAP / OAuth / SAML — only meaningful on a hosted,
// multi-user deployment. `users` / `user-groups` / `mcp-admin` / `assistants`
// are all multi-user RBAC surfaces that collapse to a no-op here.
const HIDDEN_ADMIN_ITEMS = [
  'users',
  'user-groups',
  'assistants',
  'mcp-admin',
  'auth-providers',
]

export default function SettingsPage() {
  const navigate = useNavigate()
  const location = useLocation()
  const windowMinSize = useWindowMinSize()
  const { token } = theme.useToken()

  const { slots } = Stores.ModuleSystem

  // Get and sort user settings from slots
  const userSettingsItems = (slots.get('settingsUserPages') || []).sort(
    (a, b) => (a.order ?? 0) - (b.order ?? 0),
  )

  // Get and sort admin settings from slots, filtering out hidden items
  const adminSettingsItems = (slots.get('settingsAdminPages') || [])
    .filter(item => !HIDDEN_ADMIN_ITEMS.includes(item.id))
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))

  // Build final menu (no sections in desktop app)
  const menuItems = [
    ...userSettingsItems.map(item => ({
      key: item.path,
      icon: item.icon,
      label: item.label,
    })),
    ...adminSettingsItems.map(item => ({
      key: item.path,
      icon: item.icon,
      label: item.label,
    })),
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

  const currentSection = validSections.includes(urlSection)
    ? urlSection
    : validSections[0]

  // Redirect to first available settings page if at root /settings
  useEffect(() => {
    if (
      location.pathname === '/settings' ||
      location.pathname === '/settings/'
    ) {
      if (validSections.length > 0) {
        navigate(`/settings/${validSections[0]}`, { replace: true })
      }
    }
  }, [location.pathname, navigate, validSections])

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
    <div className="h-full flex flex-col overflow-hidden">
      {/* Page Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Typography.Title level={4} className="!m-0 !leading-tight truncate">
            Settings
          </Typography.Title>
          {windowMinSize.xs && (
            <div className="flex flex-1 items-center px-2">
              <Dropdown
                overlayStyle={{
                  border: '1px solid ' + token.colorBorderSecondary,
                }}
                overlayClassName={`
                  rounded-md
                  `}
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
                <Button type="text" className={'mt-[2px]'}>
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
        {/* Desktop Sidebar */}
        {!windowMinSize.xs && (
          <div className="w-fit">
            <SettingsMenu />
          </div>
        )}

        {/* Main Content Area */}
        <div className="flex-1 overflow-hidden">
          <Outlet />
        </div>
      </div>
    </div>
  )
}

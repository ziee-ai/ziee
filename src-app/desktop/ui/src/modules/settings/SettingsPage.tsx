/**
 * DELIBERATE DIVERGENCE from core's SettingsPage.
 *
 * Why: desktop is single-admin. No need for the web's "User Settings"
 * vs "Admin Settings" sectioning, no need for permission-based filtering
 * (the bootstrapped admin has every permission), no need for the inline
 * 403 panel core renders on forbidden deep-links.
 *
 * What this file does differently:
 *   1. Collapses both `settingsUserPages` and `settingsAdminPages` into a
 *      single flat menu (no section divider, no "Admin Settings" header).
 *   2. Filters BOTH slot lists through `HIDDEN_ITEMS` — entries whose `id`
 *      is in the set never appear in the menu.
 *   3. Combined desktop modules (memory-desktop, llm-providers-desktop)
 *      register their own entry; HIDDEN_ITEMS removes the equivalent
 *      core entries so the user sees one combined "Memory" and one
 *      "LLM Providers" instead of duplicates.
 *
 * If core's SettingsPage gains a feature that ALL settings UIs need
 * (e.g. a new layout primitive), re-sync the layout shell below — keep
 * the filter list + flat menu logic.
 */

import { Button, Dropdown, Flex, Menu, theme, Typography } from 'antd'
import { Outlet, useLocation, useNavigate } from 'react-router-dom'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { IoIosArrowDown, IoMdSettings } from 'react-icons/io'
import { useEffect } from 'react'
import { Stores } from '@/core/stores'

// Slot entries (in EITHER `settingsUserPages` or `settingsAdminPages`)
// whose `id` is in this set are hidden from the desktop menu.
//
//  - Multi-user RBAC surfaces (no role on a single-admin desktop):
//    users, user-groups, assistants, mcp-admin, auth-providers.
//  - Core's user+admin pair for Memory (both register id='memory'):
//    hidden so the combined `memory-desktop` slot is the only one shown.
//  - `user-llm-providers`: lets a non-admin OVERRIDE the admin-set API
//    key with their own. On single-admin desktop there's no admin/user
//    split — the admin sets keys directly on the admin LLM Providers
//    page. The user-side entry is therefore redundant.
const HIDDEN_ITEMS = new Set([
  'users',
  'user-groups',
  'assistants',
  'mcp-admin',
  'auth-providers',
  'memory',
  'user-llm-providers',
])

export default function SettingsPage() {
  const navigate = useNavigate()
  const location = useLocation()
  const windowMinSize = useWindowMinSize()
  const { token } = theme.useToken()

  const { slots } = Stores.ModuleSystem

  // Apply HIDDEN_ITEMS to BOTH slot lists (single-admin desktop doesn't
  // care about the user/admin distinction; what matters is the id).
  const userSettingsItems = (slots.get('settingsUserPages') || [])
    .filter(item => !HIDDEN_ITEMS.has(item.id))
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))

  const adminSettingsItems = (slots.get('settingsAdminPages') || [])
    .filter(item => !HIDDEN_ITEMS.has(item.id))
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

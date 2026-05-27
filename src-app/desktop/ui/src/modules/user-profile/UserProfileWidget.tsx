/**
 * DELIBERATE DIVERGENCE from core's UserProfileWidget.
 *
 * Inherits from core (drift fix — desktop had silently dropped these):
 *   - Sidebar-collapse awareness (icon-only when collapsed, with
 *     tooltip showing the username on hover).
 *   - "Profile" dropdown entry navigating to /settings/general.
 *   - Theme-aware hover styling on the sidebar row.
 *
 * Desktop-only modification:
 *   - DROPS the "Logout" menu entry when running inside the Tauri
 *     webview. Auto-login fires at module init time only, so a
 *     logged-out desktop user would be stuck on the bootstrap
 *     spinner forever — better to not offer the trap. The widget
 *     itself is still rendered so the admin can see who they're
 *     logged in as and navigate to their profile settings. Web
 *     usage (the same bundle loaded outside Tauri) keeps Logout.
 *
 * Keep in sync with `ui/src/modules/user-profile/UserProfileWidget.tsx`;
 * `just desktop-drift-check` flags any divergence other than the
 * Tauri logout-strip.
 */

import { Dropdown, Tooltip, theme } from 'antd'
import { LogoutOutlined, UserOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { useNavigate } from 'react-router-dom'
import { isTauriView } from '@ziee/desktop/core/platform'

function SidebarItem({
  icon,
  label,
  onClick,
  collapsed,
}: {
  icon: React.ReactNode
  label: string
  onClick?: () => void
  collapsed?: boolean
}) {
  const { token } = theme.useToken()

  return (
    <div
      onClick={onClick}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer transition-colors duration-150"
      style={{ color: token.colorTextBase }}
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = token.colorPrimaryHover
        e.currentTarget.style.color = token.colorTextLightSolid
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
        e.currentTarget.style.color = token.colorTextBase
      }}
    >
      <div
        className="w-4 h-4 mr-1.5 flex items-center justify-center"
        style={{ fontSize: 18 }}
      >
        {icon}
      </div>
      <span
        style={{
          fontSize: 15,
          opacity: collapsed ? 0 : 1,
          maxWidth: collapsed ? 0 : 200,
          overflow: 'hidden',
          whiteSpace: 'nowrap',
          transition: 'opacity 200ms ease-out, max-width 200ms ease-out',
        }}
      >
        {label}
      </span>
    </div>
  )
}

export function UserProfileWidget() {
  const { user } = Stores.Auth
  const { isSidebarCollapsed } = Stores.AppLayout
  const navigate = useNavigate()

  if (!user) return null

  const menuItems = [
    {
      key: 'profile',
      icon: <UserOutlined />,
      label: 'Profile',
      onClick: () => navigate('/settings/general'),
    },
    // Logout is useless inside the Tauri webview (auto-login fires only
    // at module init time, so a logged-out desktop user would land on
    // the bootstrap spinner with no way back). Hide the entry there.
    !isTauriView && {
      key: 'logout',
      icon: <LogoutOutlined />,
      label: 'Logout',
      onClick: async () => await Stores.Auth.logoutUser(),
    },
  ].filter(Boolean) as Array<{
    key: string
    icon: React.ReactNode
    label: string
    onClick: () => void
  }>

  const item = (
    <Dropdown
      menu={{ items: menuItems }}
      placement="topLeft"
      trigger={['click']}
    >
      <div>
        <SidebarItem
          icon={<UserOutlined />}
          label={user.username}
          collapsed={isSidebarCollapsed}
        />
      </div>
    </Dropdown>
  )

  if (isSidebarCollapsed) {
    return (
      <Tooltip title={user.username} placement="right">
        {item}
      </Tooltip>
    )
  }

  return item
}

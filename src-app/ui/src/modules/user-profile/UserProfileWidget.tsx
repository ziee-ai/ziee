import { Dropdown, Skeleton, Tooltip, theme } from 'antd'
import type { MenuProps } from 'antd'
import { LogoutOutlined, UserOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { useNavigate } from 'react-router-dom'

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
      role="button"
      tabIndex={0}
      onClick={onClick}
      aria-label={label}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onClick?.()
        }
      }}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer transition-colors duration-150 focus-visible:outline focus-visible:outline-2"
      style={{ color: token.colorTextBase }}
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = token.colorPrimaryHover
        e.currentTarget.style.color = token.colorTextLightSolid
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
        e.currentTarget.style.color = token.colorTextBase
      }}
      onFocus={e => {
        e.currentTarget.style.backgroundColor = token.colorPrimaryHover
        e.currentTarget.style.color = token.colorTextLightSolid
      }}
      onBlur={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
        e.currentTarget.style.color = token.colorTextBase
      }}
    >
      <div
        aria-hidden="true"
        className="w-4 h-4 mr-1.5 flex items-center justify-center"
        style={{ fontSize: 18 }}
      >
        {icon}
      </div>
      <span
        title={label}
        style={{
          fontSize: token.fontSize,
          opacity: collapsed ? 0 : 1,
          maxWidth: collapsed ? 0 : 200,
          overflow: 'hidden',
          whiteSpace: 'nowrap',
          textOverflow: 'ellipsis',
          transition: 'opacity 200ms ease-out, max-width 200ms ease-out',
        }}
      >
        {label}
      </span>
    </div>
  )
}

export function UserProfileWidget() {
  const { user, isInitializing, isLoading } = Stores.Auth
  const { isSidebarCollapsed } = Stores.AppLayout
  const canViewProfile = usePermission(Permissions.ProfileRead)
  const navigate = useNavigate()

  if (!user) {
    // While auth is still resolving show a placeholder so the sidebar footer
    // doesn't pop in; once auth has settled with no user (logged out) render
    // nothing.
    if (isInitializing || isLoading) {
      return (
        <div data-testid="user-profile-widget-loading" className="px-2 py-1">
          <Skeleton.Avatar active size="small" shape="circle" />
        </div>
      )
    }
    return null
  }

  const item = (
    <Dropdown
      menu={{
        items: [
          canViewProfile && {
            key: 'profile',
            icon: <UserOutlined />,
            label: 'Profile',
            onClick: () => navigate('/settings/profile'),
          },
          {
            key: 'logout',
            icon: <LogoutOutlined />,
            label: 'Logout',
            onClick: async () => await Stores.Auth.logoutUser(),
          },
        ].filter(Boolean) as MenuProps['items'],
      }}
      placement="topLeft"
      trigger={['click']}
    >
      <div data-testid="user-profile-widget">
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

import { Dropdown, Tooltip, theme } from 'antd'
import { LogoutOutlined, UserOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
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

  const item = (
    <Dropdown
      menu={{
        items: [
          {
            key: 'profile',
            icon: <UserOutlined />,
            label: 'Profile',
            onClick: () => navigate('/settings/general'),
          },
          {
            key: 'logout',
            icon: <LogoutOutlined />,
            label: 'Logout',
            onClick: async () => await Stores.Auth.logoutUser(),
          },
        ].filter(Boolean),
      }}
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

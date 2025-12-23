/**
 * Desktop Override: UserProfileWidget
 *
 * - Tauri view: Hidden completely (auto-login handles auth)
 * - Web browser: Shows with logout option only
 */

import { Dropdown } from 'antd'
import { LogoutOutlined, UserOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { isTauriView } from '@ziee/desktop/core/platform'

function SidebarItem({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode
  label: string
  onClick?: () => void
}) {
  return (
    <div
      onClick={onClick}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer transition-colors duration-150"
    >
      <div className="w-4 h-4 mr-1.5 flex items-center justify-center">
        {icon}
      </div>
      <span>{label}</span>
    </div>
  )
}

export function UserProfileWidget() {
  const { user } = Stores.Auth

  // Hide in Tauri desktop app
  if (isTauriView) return null

  if (!user) return null

  // Web browser: show with logout only
  return (
    <Dropdown
      menu={{
        items: [
          {
            key: 'logout',
            icon: <LogoutOutlined />,
            label: 'Logout',
            onClick: async () => await Stores.Auth.logoutUser(),
          },
        ],
      }}
      placement="topLeft"
      trigger={['click']}
    >
      <div>
        <SidebarItem icon={<UserOutlined />} label={user.username} />
      </div>
    </Dropdown>
  )
}

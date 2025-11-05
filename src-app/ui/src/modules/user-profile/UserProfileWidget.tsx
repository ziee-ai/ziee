import { Dropdown } from 'antd'
import { LogoutOutlined, UserOutlined } from '@ant-design/icons'
import { logoutUser } from '../auth'
import { Stores } from '@/core/stores'

// Import SidebarItem from LeftSidebar - we'll need to extract this to a shared location
function SidebarItem({ icon, label, onClick }: { icon: React.ReactNode, label: string, onClick?: () => void }) {
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

  if (!user) return null

  return (
    <Dropdown
      menu={{
        items: [
          {
            key: 'profile',
            icon: <UserOutlined />,
            label: 'Profile',
            onClick: () => console.log('Profile clicked'),
          },
          {
            key: 'logout',
            icon: <LogoutOutlined />,
            label: 'Logout',
            onClick: async () => await logoutUser(),
          },
        ].filter(Boolean),
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

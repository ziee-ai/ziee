import { Link, useLocation } from 'react-router-dom'
import { Divider, Dropdown, theme, Typography } from 'antd'
import {
  AppstoreOutlined,
  FolderOutlined,
  HistoryOutlined,
  LogoutOutlined,
  PlusOutlined,
  SettingOutlined,
  UserOutlined,
} from '@ant-design/icons'
import { logoutUser } from '../../modules/auth'
import { Stores } from '../../core/stores'
import { useWindowMinSize } from '../../hooks/useWindowMinSize'
import { HiOutlineFaceSmile } from 'react-icons/hi2'

const { Text } = Typography

interface SidebarItemProps {
  icon: React.ReactNode
  label: string
  isActive?: boolean
  to?: string
  onClick?: () => void
}

function SidebarItem({ icon, label, isActive, to, onClick }: SidebarItemProps) {
  const { token } = theme.useToken()
  return (
    <Link
      to={to || '#'}
      onClick={onClick}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer transition-colors duration-150 no-underline"
      style={{
        textDecoration: 'none',
        backgroundColor: isActive ? token.colorPrimary : 'transparent',
        color: isActive ? token.colorTextLightSolid : token.colorTextBase,
        borderRadius: token.borderRadius,
      }}
      onMouseEnter={e => {
        if (!isActive) {
          e.currentTarget.style.backgroundColor = token.colorPrimaryHover
        }
      }}
      onMouseLeave={e => {
        if (!isActive) {
          e.currentTarget.style.backgroundColor = 'transparent'
        }
      }}
    >
      <div
        className="w-4 h-4 mr-1.5 flex items-center justify-center"
        style={{
          color: isActive ? token.colorTextLightSolid : token.colorTextBase,
          transition: 'color 0.15s ease',
        }}
      >
        {icon}
      </div>
      <Text style={{ color: 'inherit' }}>{label}</Text>
    </Link>
  )
}

interface SectionHeaderProps {
  children: React.ReactNode
}

function SectionHeader({ children }: SectionHeaderProps) {
  const { token } = theme.useToken()
  return (
    <Text
      className="px-3 pb-0.5 block font-semibold tracking-wide"
      style={{
        fontSize: '11px',
        color: token.colorTextSecondary,
      }}
    >
      {children}
    </Text>
  )
}

export function LeftSidebar() {
  const location = useLocation()
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()

  const { user } = Stores.Auth

  const isActive = (path: string) => {
    if (path === '/conversations')
      return location.pathname.startsWith('/conversations')
    if (path === '/projects') return location.pathname.startsWith('/projects')
    if (path === '/artifacts') return location.pathname.startsWith('/artifacts')
    if (path === '/hub') return location.pathname.startsWith('/hub')
    if (path === '/assistants')
      return location.pathname.startsWith('/assistants')
    if (path === '/settings') return location.pathname.startsWith('/settings')
    return false
  }

  return (
    <div
      className="h-full flex flex-col overflow-hidden"
      style={{
        width: '100%', // Take full width of container
        borderRight: windowMinSize.xs
          ? 'none'
          : '1px solid ' + token.colorBorderSecondary,
        backgroundColor: token.colorBgContainer,
      }}
    >
      <div className={'h-[50px]'} />
      {/* Sidebar content - always rendered */}
      {/* New Chat Button */}

      <div className="mb-4">
        <SidebarItem icon={<PlusOutlined />} label="New Chat" to="/" />
      </div>

      {/* Navigation Section */}
      <div className="mb-4">
        <SectionHeader>Navigation</SectionHeader>
        <div className="space-y-0">
          <SidebarItem
            icon={<HistoryOutlined />}
            label="Chats"
            isActive={isActive('/conversations')}
            to="/conversations"
          />
          <SidebarItem
            icon={<FolderOutlined />}
            label="Projects"
            isActive={isActive('/projects')}
            to="/projects"
          />
        </div>
      </div>

      {/* Recent Section - Placeholder for future module injection */}
      <div className="flex-1 overflow-hidden flex flex-col">
        <SectionHeader>Recent</SectionHeader>
        {/* RecentConversations will be injected by module here */}
      </div>

      {/* Tools Section */}
      <div>
        <SectionHeader>Tools</SectionHeader>
        <div className="space-y-0 mb-2">
          <SidebarItem
            icon={<AppstoreOutlined />}
            label="Hub"
            isActive={isActive('/hub')}
            to="/hub"
          />
          <SidebarItem
            icon={<HiOutlineFaceSmile />}
            label="Assistants"
            isActive={isActive('/assistants')}
            to="/assistants"
          />
          <SidebarItem
            icon={<SettingOutlined />}
            label="Settings"
            isActive={isActive('/settings')}
            to="/settings"
          />
        </div>

        {/* Download Indicator - Placeholder for future module injection */}
        <div className="px-2">
          {/* DownloadIndicator will be injected by module here */}
        </div>

        {/* User Profile Section */}
        {user && (
          <div>
            <Divider className={'!m-0'} />
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
          </div>
        )}
      </div>
    </div>
  )
}

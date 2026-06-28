import { Dropdown, Tooltip } from '@/components/ui'
import { LogOut, User } from 'lucide-react'
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
  return (
    <div
      onClick={onClick}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer transition-colors duration-150 text-foreground"
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = 'hsl(var(--primary) / 0.9)'
        e.currentTarget.style.color = 'white'
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
        e.currentTarget.style.color = ''
      }}
    >
      <div
        className="w-4 h-4 mr-1.5 flex items-center justify-center"
        style={{ fontSize: 18 }}
      >
        {icon}
      </div>
      <span
        className="text-sm"
        style={{
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
  const canViewProfile = usePermission(Permissions.ProfileRead)
  const navigate = useNavigate()

  if (!user) return null

  const items = [
    ...(canViewProfile
      ? [
          {
            key: 'profile',
            icon: <User />,
            label: 'Profile',
            onClick: () => navigate('/settings/profile'),
          },
        ]
      : []),
    {
      key: 'logout',
      icon: <LogOut />,
      label: 'Logout',
      onClick: async () => await Stores.Auth.logoutUser(),
    },
  ]

  const item = (
    <Dropdown data-testid="userprofile-menu-dropdown" items={items} side="top" align="start">
      <div data-testid="user-profile-widget">
        <SidebarItem
          icon={<User />}
          label={user.username}
          collapsed={isSidebarCollapsed}
        />
      </div>
    </Dropdown>
  )

  if (isSidebarCollapsed) {
    return (
      <Tooltip content={user.username} side="right">
        {item}
      </Tooltip>
    )
  }

  return item
}

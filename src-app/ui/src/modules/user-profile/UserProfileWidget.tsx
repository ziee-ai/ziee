import { Dropdown, Tooltip, Skeleton } from '@ziee/kit'
import { LogOut, User } from 'lucide-react'
import { Stores } from '@ziee/framework/stores'
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
      role="button"
      tabIndex={0}
      aria-label={label}
      onClick={onClick}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          onClick?.()
        }
      }}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer transition-colors duration-150 text-foreground focus-visible:outline focus-visible:outline-2"
      onMouseEnter={e => {
        e.currentTarget.style.backgroundColor = 'color-mix(in oklab, var(--primary) 90%, transparent)'
        e.currentTarget.style.color = 'white'
      }}
      onMouseLeave={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
        e.currentTarget.style.color = ''
      }}
      onFocus={e => {
        e.currentTarget.style.backgroundColor = 'color-mix(in oklab, var(--primary) 90%, transparent)'
        e.currentTarget.style.color = 'white'
      }}
      onBlur={e => {
        e.currentTarget.style.backgroundColor = 'transparent'
        e.currentTarget.style.color = ''
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
  const { user, isInitializing, isLoading } = Stores.Auth
  const { isSidebarCollapsed } = Stores.AppLayout
  const canViewProfile = usePermission(Permissions.ProfileRead)
  const navigate = useNavigate()

  if (!user) {
    // While auth is still resolving (user not yet hydrated) show a skeleton
    // row that mirrors the SidebarItem shape — an avatar-shaped circle plus a
    // label line, composed from the kit Skeleton primitive (no Skeleton.Avatar
    // in the kit) — so the sidebar footer doesn't pop in blank then jump to the
    // profile entry. Once auth has SETTLED with no user (logged out), the
    // signals clear and we render nothing.
    if (isInitializing || isLoading) {
      return (
        <div
          data-testid="user-profile-widget-loading"
          className="flex items-center px-3 py-1 mx-2"
          aria-hidden="true"
        >
          <Skeleton className="w-4 h-4 mr-1.5 rounded-full shrink-0" />
          {!isSidebarCollapsed && <Skeleton className="h-3.5 w-24 rounded" />}
        </div>
      )
    }
    return null
  }

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

  // The sidebar identifies the PERSON, not the login handle. `||` (not `??`):
  // `display_name` can be absent, JSON null, OR a blank string — self-service
  // edits normalize blanks to NULL, but the admin create/update path stores
  // what it is given, so `'   '` is reachable. `??` would keep it and render a
  // nameless row. One const so the visible label, its aria-label/title, and the
  // collapsed tooltip can't diverge.
  const label = user.display_name?.trim() || user.username

  // role=button so the dropdown's injected aria-expanded/aria-haspopup are
  // valid on this trigger (a bare <div> doesn't support them → axe
  // aria-allowed-attr). tabIndex keeps it keyboard-reachable.
  const trigger = (
    <div data-testid="user-profile-widget" role="button" tabIndex={0}>
      <SidebarItem
        icon={<User />}
        label={label}
        collapsed={isSidebarCollapsed}
      />
    </div>
  )

  return (
    <Dropdown
      data-testid="userprofile-menu-dropdown"
      items={items}
      side="top"
      align="start"
      // Keep the menu left-aligned under the username instead of flipping to the
      // right at the sidebar's left edge (Base UI's default align collision).
      collisionAvoidance={{ align: 'shift' }}
    >
      {/* Collapsed, the label span is width-0, so the tooltip is the only thing
          naming the user — it has to actually open. The kit Tooltip must sit
          INSIDE the trigger (it forwards a parent trigger's injected props onto
          its child via Slot). Wrapping it AROUND <Dropdown> silently dropped
          them, because Dropdown destructures a fixed prop list with no
          rest-spread and never forwards unknown props to the DOM — which is why
          this tooltip previously never appeared on hover. */}
      {isSidebarCollapsed ? (
        <Tooltip content={label} side="right">
          {trigger}
        </Tooltip>
      ) : (
        trigger
      )}
    </Dropdown>
  )
}

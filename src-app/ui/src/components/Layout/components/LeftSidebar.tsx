import { Link, useLocation } from 'react-router-dom'
import { theme, Typography, Divider } from 'antd'
import { useWindowMinSize } from '@/hooks/useWindowMinSize'
import { useRouterStore } from '@/core/router'

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

  const { sidebarItems } = useRouterStore()

  const isActive = (path: string) => {
    return location.pathname.startsWith(path)
  }

  // Sort items by order
  const sortedPrimaryActions = [...sidebarItems.primaryActions].sort(
    (a, b) => (a.order ?? 0) - (b.order ?? 0),
  )
  const sortedNavigation = [...sidebarItems.navigation].sort(
    (a, b) => (a.order ?? 0) - (b.order ?? 0),
  )
  const sortedTools = [...sidebarItems.tools].sort(
    (a, b) => (a.order ?? 0) - (b.order ?? 0),
  )

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

      {/* Primary Actions */}
      {sortedPrimaryActions.length > 0 && (
        <div className="mb-4">
          {sortedPrimaryActions.map(action => (
            <SidebarItem
              key={action.id}
              icon={action.icon}
              label={action.label}
              to={action.to}
              onClick={action.onClick}
            />
          ))}
        </div>
      )}

      {/* Navigation Section */}
      {sortedNavigation.length > 0 && (
        <div className="mb-4">
          <SectionHeader>Navigation</SectionHeader>
          <div className="space-y-0">
            {sortedNavigation.map(item => (
              <SidebarItem
                key={item.id}
                icon={item.icon}
                label={item.label}
                isActive={isActive(item.path)}
                to={item.path}
              />
            ))}
          </div>
        </div>
      )}

      {/* Recent Section - Widget Slot */}
      <div className="flex-1 overflow-hidden flex flex-col">
        {sidebarItems.widgets.has('recent') && (
          <SectionHeader>Recent</SectionHeader>
        )}
        {sidebarItems.widgets
          .get('recent')
          ?.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
          .map(widget => (
            <div key={widget.id}>{widget.component}</div>
          ))}
      </div>

      {/* Tools Section */}
      {sortedTools.length > 0 && (
        <div>
          <SectionHeader>Tools</SectionHeader>
          <div className="space-y-0 mb-2">
            {sortedTools.map(item => (
              <SidebarItem
                key={item.id}
                icon={item.icon}
                label={item.label}
                isActive={isActive(item.path)}
                to={item.path}
              />
            ))}
          </div>

          {/* Bottom Widgets */}
          <div className="px-2">
            {sidebarItems.widgets
              .get('bottom')
              ?.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
              .map(widget => (
                <div key={widget.id}>{widget.component}</div>
              ))}
          </div>
        </div>
      )}

      {/* Footer Slot */}
      {sidebarItems.widgets.has('footer') &&
        sidebarItems.widgets.get('footer')!.length > 0 && (
          <>
            <Divider className="!m-0" />
            {sidebarItems.widgets
              .get('footer')
              ?.sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
              .map(widget => (
                <div key={widget.id}>{widget.component}</div>
              ))}
          </>
        )}
    </div>
  )
}

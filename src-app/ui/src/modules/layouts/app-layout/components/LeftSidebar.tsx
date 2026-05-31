import { Link, useLocation } from 'react-router-dom'
import { theme, Typography, Divider, Tooltip } from 'antd'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { SidebarHeaderSpacer } from '@/modules/layouts/app-layout/components/SidebarHeaderSpacer'
import { Stores } from '@/core/stores'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { evaluatePermission } from '@/core/permissions'
import type {
  SidebarNavItem,
  SidebarToolItem,
  SidebarActionItem,
} from '@/modules/layouts/app-layout/types'

const { Text } = Typography

interface SidebarItemProps {
  icon: React.ReactNode
  label: string
  isActive?: boolean
  to?: string
  onClick?: () => void
  collapsed?: boolean
}

function SidebarItem({ icon, label, isActive, to, onClick, collapsed }: SidebarItemProps) {
  const { token } = theme.useToken()

  const item = (
    <Link
      to={to || '#'}
      onClick={onClick}
      className="flex items-center px-3 py-1 mx-2 rounded-md cursor-pointer no-underline"
      style={{
        textDecoration: 'none',
        backgroundColor: isActive ? token.colorPrimary : 'transparent',
        color: isActive ? token.colorTextLightSolid : token.colorTextBase,
        borderRadius: token.borderRadius,
        transition: 'background-color 150ms, color 150ms',
      }}
      onMouseEnter={e => {
        if (!isActive) {
          e.currentTarget.style.backgroundColor = token.colorPrimaryHover
          e.currentTarget.style.color = token.colorTextLightSolid
        }
      }}
      onMouseLeave={e => {
        if (!isActive) {
          e.currentTarget.style.backgroundColor = 'transparent'
          e.currentTarget.style.color = token.colorTextBase
        }
      }}
    >
      <div
        className="w-4 h-4 mr-1.5 flex items-center justify-center"
        style={{
          fontSize: 18,
        }}
        aria-hidden="true"
      >
        {icon}
      </div>
      <Text
        style={{
          color: 'inherit',
          fontSize: token.fontSize,
          opacity: collapsed ? 0 : 1,
          maxWidth: collapsed ? 0 : 200,
          overflow: 'hidden',
          whiteSpace: 'nowrap',
          transition: 'opacity 200ms ease-out, max-width 200ms ease-out',
        }}
      >
        {label}
      </Text>
    </Link>
  )

  if (collapsed) {
    return (
      <Tooltip title={label} placement="right">
        {item}
      </Tooltip>
    )
  }

  return item
}

interface SectionHeaderProps {
  children: React.ReactNode
  collapsed?: boolean
}

function SectionHeader({ children, collapsed }: SectionHeaderProps) {
  const { token } = theme.useToken()

  return (
    <div
      style={{
        maxHeight: collapsed ? 0 : 32,
        opacity: collapsed ? 0 : 1,
        overflow: 'hidden',
        transition: 'opacity 200ms ease-out, max-height 200ms ease-out',
      }}
    >
      <Text
        className="px-3 pb-0.5 block font-semibold tracking-wide"
        style={{
          fontSize: token.fontSizeSM,
          color: token.colorTextSecondary,
        }}
      >
        {children}
      </Text>
    </div>
  )
}

export function LeftSidebar() {
  const location = useLocation()
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()
  const { slots } = Stores.ModuleSystem
  const { isSidebarCollapsed } = Stores.AppLayout
  const { user, permissions } = Stores.Auth

  const isActive = (path: string) => {
    return location.pathname.startsWith(path)
  }

  const isAllowed = (item: { permission?: SidebarNavItem['permission'] }) =>
    !item.permission || evaluatePermission(user, permissions, item.permission)

  // Get and sort items from slots
  const primaryActions = (slots.get('sidebarPrimaryActions') ||
    []) as SidebarActionItem[]
  const navigation = (slots.get('sidebarNavigation') || []) as SidebarNavItem[]
  const tools = (slots.get('sidebarTools') || []) as SidebarToolItem[]
  const contentWidgets = slots.get('sidebarContent') || []
  const bottomWidgets = slots.get('sidebarBottom') || []
  const footerWidgets = slots.get('sidebarFooter') || []

  const sortedPrimaryActions = [...primaryActions].sort(
    (a, b) => (a.order ?? 0) - (b.order ?? 0),
  )
  const sortedNavigation = [...navigation]
    .filter(isAllowed)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
  const sortedTools = [...tools]
    .filter(isAllowed)
    .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))

  // On desktop, collapsed means icon-only mode (not fully hidden)
  const isIconOnly = isSidebarCollapsed && !windowMinSize.xs

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
      <SidebarHeaderSpacer />
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
              collapsed={isIconOnly}
            />
          ))}
        </div>
      )}

      {/* Navigation Section */}
      {sortedNavigation.length > 0 && (
        <div className="mb-4">
          <SectionHeader collapsed={isIconOnly}>Navigation</SectionHeader>
          <div className="space-y-0">
            {sortedNavigation.map(item => (
              <SidebarItem
                key={item.id}
                icon={item.icon}
                label={item.label}
                isActive={isActive(item.path)}
                to={item.path}
                collapsed={isIconOnly}
              />
            ))}
          </div>
        </div>
      )}

      {/* Content Section - Widget Slot (hidden in icon-only mode) */}
      {!isIconOnly && (
        <div className="flex-1 overflow-hidden flex flex-col">
          {contentWidgets
            .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
            .map(widget => (
              <div key={widget.id} className="flex-1 min-h-0 flex flex-col">
                <LazyComponentRenderer component={widget.component} />
              </div>
            ))}
        </div>
      )}

      {/* Spacer in icon-only mode to push tools to bottom */}
      {isIconOnly && <div className="flex-1" />}

      {/* Tools Section */}
      {sortedTools.length > 0 && (
        <div>
          <SectionHeader collapsed={isIconOnly}>Tools</SectionHeader>
          <div className="space-y-0 mb-2">
            {sortedTools.map(item => (
              <SidebarItem
                key={item.id}
                icon={item.icon}
                label={item.label}
                isActive={isActive(item.path)}
                to={item.path}
                collapsed={isIconOnly}
              />
            ))}
          </div>

          {/* Bottom Widgets (hidden in icon-only mode) */}
          {!isIconOnly && bottomWidgets.length > 0 && (
            <div className="px-2">
              {bottomWidgets
                .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
                .map(widget => (
                  <div key={widget.id}>
                    <LazyComponentRenderer component={widget.component} />
                  </div>
                ))}
            </div>
          )}
        </div>
      )}

      {/* Footer Slot */}
      {footerWidgets.length > 0 && (
        <>
          <Divider className="!m-0" />
          <div className="py-2">
            {footerWidgets
              .sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
              .map(widget => (
                <div key={widget.id}>
                  <LazyComponentRenderer component={widget.component} />
                </div>
              ))}
          </div>
        </>
      )}
    </div>
  )
}

import { useNavigate, useLocation } from 'react-router-dom'
import { Divider, Menu, theme } from 'antd'
import type { MenuProps } from 'antd'
import type { CSSProperties } from 'react'
import { useMemo } from 'react'
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

type MenuItem = NonNullable<MenuProps['items']>[number]

/**
 * Pick the most-specific item whose `path` is a prefix of the current
 * pathname. Returns the item's id (the Menu's `key`) or undefined when
 * nothing in this group is active. Matches the original sidebar's
 * "startsWith" semantics so submenu pages keep their parent
 * highlighted (e.g. `/settings/profile` highlights "Settings").
 */
function selectedKeyForGroup(
  pathname: string,
  items: { id: string; path: string }[],
): string | undefined {
  let best: { id: string; pathLen: number } | undefined
  for (const it of items) {
    if (pathname === it.path || pathname.startsWith(it.path + '/')) {
      if (!best || it.path.length > best.pathLen) {
        best = { id: it.id, pathLen: it.path.length }
      }
    }
  }
  return best?.id
}

/**
 * Optional shape that lets an outer wrapper (typically a platform-
 * specific build override) tweak the sidebar's outer chrome without
 * forking the whole component. Default values come from theme tokens.
 *
 * Both `rootStyle` and `rootClassName` are spread / appended AFTER
 * the defaults so the override always wins.
 */
interface LeftSidebarProps {
  rootStyle?: CSSProperties
  rootClassName?: string
}

export function LeftSidebar({ rootStyle, rootClassName }: LeftSidebarProps = {}) {
  const navigate = useNavigate()
  const location = useLocation()
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()
  const { slots } = Stores.ModuleSystem
  const { isSidebarCollapsed } = Stores.AppLayout
  const { user, permissions } = Stores.Auth

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

  const sortedPrimaryActions = useMemo(
    () => [...primaryActions].sort((a, b) => (a.order ?? 0) - (b.order ?? 0)),
    [primaryActions],
  )
  const sortedNavigation = useMemo(
    () =>
      [...navigation]
        .filter(isAllowed)
        .sort((a, b) => (a.order ?? 0) - (b.order ?? 0)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [navigation, user, permissions],
  )
  const sortedTools = useMemo(
    () =>
      [...tools]
        .filter(isAllowed)
        .sort((a, b) => (a.order ?? 0) - (b.order ?? 0)),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [tools, user, permissions],
  )

  // On desktop, collapsed means icon-only mode (not fully hidden)
  const isIconOnly = isSidebarCollapsed && !windowMinSize.xs

  // Build antd Menu item lists per group. Action / tool / nav are
  // separate Menus because the widget slots (`sidebarContent`,
  // `sidebarBottom`, `sidebarFooter`) sit BETWEEN them as their own
  // flow elements. Within each Menu, antd handles hover/active/focus
  // tokens, keyboard nav, and the icon-only tooltip on its own.
  const primaryItems: MenuItem[] = sortedPrimaryActions.map(a => ({
    key: a.id,
    icon: a.icon,
    label: a.label,
  }))

  const navigationItems: MenuItem[] = sortedNavigation.map(n => ({
    key: n.id,
    icon: n.icon,
    label: n.label,
  }))

  const toolsItems: MenuItem[] = sortedTools.map(t => ({
    key: t.id,
    icon: t.icon,
    label: t.label,
  }))

  const navSelectedKey = selectedKeyForGroup(location.pathname, sortedNavigation)
  const toolsSelectedKey = selectedKeyForGroup(location.pathname, sortedTools)
  // Primary actions surface paths only when present (otherwise the
  // action is a pure `onClick` like "New Chat" which never has a
  // selected state). Build the list with non-null paths only.
  const primarySelectedKey = selectedKeyForGroup(
    location.pathname,
    sortedPrimaryActions
      .filter((a): a is SidebarActionItem & { to: string } => Boolean(a.to))
      .map(a => ({ id: a.id, path: a.to })),
  )

  // Shared Menu props — transparent background to inherit the sidebar
  // surface, no right border (the sidebar container owns its own
  // border). Items keep antd's default margin-from-panel +
  // rounded-pill selection look, but the CONTENT inside each item is
  // tight: 28px row, 8px horizontal padding (vs antd's default ~16px).
  const menuClass =
    '!bg-transparent !border-none ' +
    '[&_.ant-menu-item]:!h-7 [&_.ant-menu-item]:!leading-[28px] ' +
    // Force BOTH width and margin together so antd's own
    // `width: calc(100% - 8px)` and small default margin can't
    // re-assert — without the width override, an `mx-*` on the item
    // pushes the left in but the right keeps overflowing to the
    // box edge. Width: 100% minus the 16px combined margin (8px
    // each side) puts the right edge exactly where we want.
    '[&_.ant-menu-item]:!mx-2 ' +
    '[&_.ant-menu-item]:!w-[calc(100%-1rem)] ' +
    '[&_.ant-menu-item]:!pl-2 [&_.ant-menu-item]:!pr-2 ' +
    '[&_.ant-menu-item]:!py-0 ' +
    '[&_.ant-menu-item]:!rounded-md ' +
    '[&_.ant-menu-title-content]:!py-0 ' +
    '[&_.ant-menu-item-group-title]:!px-3 [&_.ant-menu-item-group-title]:!pt-0 ' +
    '[&_.ant-menu-item-group-title]:!pb-0.5 ' +
    '[&_.ant-menu-item-group-title]:!text-xs ' +
    '[&_.ant-menu-item-group-title]:!font-semibold ' +
    '[&_.ant-menu-item-group-title]:!tracking-wide'

  const handleNavMenuClick = (key: string) => {
    const item = sortedNavigation.find(n => n.id === key)
    if (item) navigate(item.path)
  }

  const handleToolsMenuClick = (key: string) => {
    const item = sortedTools.find(t => t.id === key)
    if (item) navigate(item.path)
  }

  const handlePrimaryMenuClick = (key: string) => {
    const item = sortedPrimaryActions.find(a => a.id === key)
    if (!item) return
    if (item.onClick) item.onClick()
    if (item.to) navigate(item.to)
  }

  return (
    <div
      className={
        'h-full flex flex-col overflow-hidden' +
        (rootClassName ? ' ' + rootClassName : '')
      }
      style={{
        width: '100%',
        borderRight: windowMinSize.xs
          ? 'none'
          : '1px solid ' + token.colorBorderSecondary,
        // Sidebar uses the layout surface (off-white) so it reads as
        // a fractionally recessed panel against the main content
        // pane (`colorBgContainer`, pure white).
        backgroundColor: token.colorBgLayout,
        // Wrapper overrides win — applied last.
        ...rootStyle,
      }}
    >
      <SidebarHeaderSpacer />

      {/* Primary Actions — no section header, like the original. */}
      {primaryItems.length > 0 && (
        <Menu
          mode="inline"
          inlineCollapsed={isIconOnly}
          className={menuClass}
          selectedKeys={primarySelectedKey ? [primarySelectedKey] : []}
          items={primaryItems}
          onClick={({ key }) => handlePrimaryMenuClick(String(key))}
        />
      )}

      {/* Navigation — section header rendered as a Menu item group so
          antd handles the collapsed-mode hide + token-based typography. */}
      {navigationItems.length > 0 && (
        <div className="mt-2">
          <Menu
            mode="inline"
            inlineCollapsed={isIconOnly}
            className={menuClass}
            selectedKeys={navSelectedKey ? [navSelectedKey] : []}
            items={[
              {
                type: 'group',
                label: 'Navigation',
                children: navigationItems,
              },
            ]}
            onClick={({ key }) => handleNavMenuClick(String(key))}
          />
        </div>
      )}

      {/* Content Section — widget slot (hidden in icon-only mode). */}
      {!isIconOnly && (
        <div className="flex-1 overflow-hidden flex flex-col mt-2">
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

      {/* Tools — same shape as Navigation. */}
      {toolsItems.length > 0 && (
        <div className="mt-2">
          <Menu
            mode="inline"
            inlineCollapsed={isIconOnly}
            className={menuClass}
            selectedKeys={toolsSelectedKey ? [toolsSelectedKey] : []}
            items={[
              {
                type: 'group',
                label: 'Tools',
                children: toolsItems,
              },
            ]}
            onClick={({ key }) => handleToolsMenuClick(String(key))}
          />

          {/* Bottom Widgets (hidden in icon-only mode) */}
          {!isIconOnly && bottomWidgets.length > 0 && (
            <div className="px-2 mt-2">
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

import { useNavigate, useLocation } from 'react-router-dom'
import type { CSSProperties } from 'react'
import { useMemo } from 'react'
import { Menu, Separator } from '@ziee/kit'
import type { MenuItem } from '@ziee/kit'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { SidebarHeaderSpacer } from '@/modules/layouts/app-layout/components/SidebarHeaderSpacer'
import { SidebarSectionTitle } from '@/components/common/SidebarSectionTitle'
import { Stores } from '@ziee/framework/stores'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { evaluatePermission } from '@/core/permissions'
import type {
  SidebarNavItem,
  SidebarToolItem,
  SidebarActionItem,
} from '@/modules/layouts/app-layout/types'

/**
 * Split a group's active state into the EXACT current page vs. a broader
 * ANCESTOR section. `exact` is the item whose `path` equals the pathname (the
 * strong selected pill); `ancestor` is the most-specific item whose `path` is a
 * proper prefix (a subtle "you're in this section" treatment, e.g.
 * `/settings/profile` softly marks "Settings", and a project conversation softly
 * marks "Projects" while the Recent-chats list owns the strong current mark).
 * Never returns both — an exact match wins outright.
 */
function selectionForGroup(
  pathname: string,
  items: { id: string; path: string }[],
): { exact?: string; ancestor?: string } {
  let exact: string | undefined
  let ancestor: { id: string; pathLen: number } | undefined
  for (const it of items) {
    if (pathname === it.path) {
      exact = it.id
    } else if (pathname.startsWith(it.path + '/')) {
      // A broader section you're within (e.g. "Projects" while viewing a project
      // conversation at /projects/:id/chat/:id). Keep the most-specific one.
      if (!ancestor || it.path.length > ancestor.pathLen) {
        ancestor = { id: it.id, pathLen: it.path.length }
      }
    }
  }
  // Only the EXACT current page gets the strong selected pill. When the page is a
  // sub-route (a conversation the Recent-chats list already marks current, a
  // settings subpage, …) the section item is a subtle ancestor, not a second
  // "current". Exact wins outright — no ancestor is surfaced alongside it.
  return { exact, ancestor: exact ? undefined : ancestor?.id }
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
  // Widget slots carry an optional `permission` (like nav/tool items).
  // Filter here so a widget backed by permission-restricted data (recent
  // conversations, download indicator) never mounts — and never fires its
  // on-mount fetch — for a user who lacks the grant. Widgets with no
  // `permission` render unconditionally (they self-gate internally if
  // needed). Reactive via the `user`/`permissions` read above.
  const contentWidgets = (slots.get('sidebarContent') || []).filter(isAllowed)
  const bottomWidgets = (slots.get('sidebarBottom') || []).filter(isAllowed)
  const footerWidgets = (slots.get('sidebarFooter') || []).filter(isAllowed)

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

  // Build kit Menu item lists per group. In icon-only mode, labels are
  // omitted so only the icon is rendered (kit Menu has no inlineCollapsed
  // equivalent; tooltips on hover are not available in icon-only mode).
  const primaryItems: MenuItem[] = sortedPrimaryActions.map(a => ({
    key: a.id,
    icon: a.icon,
    label: isIconOnly ? null : a.label,
  }))

  const navigationItems: MenuItem[] = sortedNavigation.map(n => ({
    key: n.id,
    icon: n.icon,
    label: isIconOnly ? null : n.label,
  }))

  const toolsItems: MenuItem[] = sortedTools.map(t => ({
    key: t.id,
    icon: t.icon,
    label: isIconOnly ? null : t.label,
  }))

  const navSel = selectionForGroup(location.pathname, sortedNavigation)
  const toolsSel = selectionForGroup(location.pathname, sortedTools)
  // Primary actions surface paths only when present (otherwise the
  // action is a pure `onClick` like "New Chat" which never has a
  // selected state). Build the list with non-null paths only.
  const primarySel = selectionForGroup(
    location.pathname,
    sortedPrimaryActions
      .filter((a): a is SidebarActionItem & { to: string } => Boolean(a.to))
      .map(a => ({ id: a.id, path: a.to })),
  )
  const ancestorList = (id?: string) => (id ? [id] : undefined)

  // On mobile the sidebar is a full-screen Sheet and AppLayout remounts on each
  // route change — so navigating away while the Sheet is open unmounts it mid-
  // open and orphans its portaled overlay in <body>, which then swallows every
  // tap on the new page. Collapse first (closing the Sheet cleanly) before we
  // navigate. No-op on desktop where the sidebar is persistent.
  const navTo = (path: string) => {
    if (windowMinSize.xs) Stores.AppLayout.setSidebarCollapsed(true)
    navigate(path)
  }

  const handleNavMenuClick = (key: string) => {
    const item = sortedNavigation.find(n => n.id === key)
    if (item) navTo(item.path)
  }

  const handleToolsMenuClick = (key: string) => {
    const item = sortedTools.find(t => t.id === key)
    if (item) navTo(item.path)
  }

  const handlePrimaryMenuClick = (key: string) => {
    const item = sortedPrimaryActions.find(a => a.id === key)
    if (!item) return
    if (item.onClick) item.onClick()
    if (item.to) navTo(item.to)
  }

  return (
    <div
      className={
        'h-full flex flex-col overflow-hidden bg-muted/40' +
        (windowMinSize.xs ? '' : ' border-r border-border') +
        (rootClassName ? ' ' + rootClassName : '')
      }
      style={{
        width: '100%',
        // Wrapper overrides win — applied last.
        ...rootStyle,
      }}
    >
      <SidebarHeaderSpacer />

      {/* Primary Actions — no section header, like the original. */}
      {primaryItems.length > 0 && (
        <Menu
          mode="vertical"
          aria-label="Primary actions"
          data-testid="layout-sidebar-primary-actions-menu"
          selectedKey={primarySel.exact}
          ancestorKeys={ancestorList(primarySel.ancestor)}
          items={primaryItems}
          onSelect={handlePrimaryMenuClick}
          className="px-2"
        />
      )}

      {/* Navigation — the caption is a SIBLING of the Menu, not a kit Menu item
          group. A group title's inset is this Menu's `px-2` PLUS the kit's own
          hardcoded group-title `px-3` (= 20px), which pushed it 8px right of the
          "Recent chats" caption below. Rendering the shared SidebarSectionTitle
          alongside a FLAT menu removes that stacked padding at the source rather
          than overriding the kit, and puts all three captions on one edge.
          (`sortedNavigation` is already flat — the group only ever existed to
          draw this title.) */}
      {navigationItems.length > 0 && (
        <div className="mt-2">
          <SidebarSectionTitle data-testid="layout-sidebar-nav-title">
            Navigation
          </SidebarSectionTitle>
          <Menu
            mode="vertical"
            aria-label="Primary navigation"
            data-testid="layout-sidebar-nav-menu"
            selectedKey={navSel.exact}
            ancestorKeys={ancestorList(navSel.ancestor)}
            items={navigationItems}
            onSelect={handleNavMenuClick}
            className="px-2"
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
          <SidebarSectionTitle data-testid="layout-sidebar-tools-title">
            Tools
          </SidebarSectionTitle>
          <Menu
            mode="vertical"
            aria-label="Tools navigation"
            data-testid="layout-sidebar-tools-menu"
            selectedKey={toolsSel.exact}
            ancestorKeys={ancestorList(toolsSel.ancestor)}
            items={toolsItems}
            onSelect={handleToolsMenuClick}
            className="px-2"
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
          <Separator className="my-0" />
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

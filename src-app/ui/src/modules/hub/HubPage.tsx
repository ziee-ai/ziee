import { useState, useMemo, useEffect, useRef } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import {
  Button,
  Dropdown,
  Flex,
  Result,
  Tag,
  Tooltip,
  Title,
  Menu,
  ScrollArea,
  message,
} from '@ziee/kit'
import type { MenuItem } from '@ziee/kit/kit/menu'
import { RotateCw } from 'lucide-react'
import { IoIosArrowDown } from 'react-icons/io'
import { Stores } from '@ziee/framework/stores'
import { evaluatePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { cn } from '@/lib/utils'
import { useElementMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { DivScrollY } from '@/components/common/DivScrollY'

export function HubPage() {
  const { activeTab: urlActiveTab } = useParams()
  const navigate = useNavigate()
  const { slots } = Stores.ModuleSystem
  const { user, permissions } = Stores.Auth
  // Subscribe to the MCP policy so the MCP tab's shouldRender gate re-evaluates
  // the moment an admin saves a new policy (its presence in visibleTabs deps is
  // the load-bearing piece).
  const { policy: mcpPolicy } = Stores.McpUserPolicy
  // Layout flips from a left side-menu to a header dropdown based on the page's
  // OWN width (mirrors the Settings page), not the viewport.
  const containerRef = useRef<HTMLDivElement>(null)
  const minSize = useElementMinSize(containerRef)
  const useMobileLayout = minSize.sm
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  // Native document-scroll on mobile (iOS toolbar collapse + under-notch flow).
  useNativeScroll(true)
  const { nativeScroll } = Stores.AppLayout
  const [refreshing, setRefreshing] = useState(false)

  // Get hub tabs from slot system, sorted
  const hubTabs = useMemo(() => {
    return (slots.get('hubTabs') || []).sort((a, b) => a.order - b.order)
  }, [slots])

  // Filter to tabs the current user has read permission on AND whose optional
  // `shouldRender` gate returns true. `mcpPolicy` in deps is what makes the MCP
  // tab re-evaluate when the admin policy changes.
  const visibleTabs = useMemo(
    () =>
      hubTabs.filter(
        t =>
          evaluatePermission(user, permissions, t.permissions.read) &&
          (t.shouldRender ? t.shouldRender() : true),
      ),
    [hubTabs, user, permissions, mcpPolicy],
  )

  // Default to first tab if none selected
  const activeTab = urlActiveTab || visibleTabs[0]?.id

  // Redirect to first visible tab if at /hub with no segment. Skip when there
  // are no visible tabs OR the user deep-linked to a forbidden tab (403 shown).
  const hasUrlSegment = !!urlActiveTab
  const urlSegmentIsRegistered =
    hasUrlSegment && hubTabs.some(t => t.id === urlActiveTab)
  const urlSegmentIsForbidden =
    urlSegmentIsRegistered && !visibleTabs.some(t => t.id === urlActiveTab)

  useEffect(() => {
    if (!hasUrlSegment && visibleTabs.length > 0 && !urlSegmentIsForbidden) {
      navigate(`/hub/${visibleTabs[0].id}`, { replace: true })
    }
  }, [hasUrlSegment, visibleTabs, navigate, urlSegmentIsForbidden])

  const currentTabSlot = visibleTabs.find(t => t.id === activeTab)
  // Page-level Refresh is admin-only (calls POST /api/hub/refresh — fetches the
  // latest signed catalog, sha256 + cosign verifies, atomic rotate).
  const canRefresh = evaluatePermission(
    user,
    permissions,
    Permissions.HubCatalogManage,
  )
  const hubVersion = Stores.HubCatalog.hubVersion
  const serverVersion = Stores.HubCatalog.serverVersion

  const handleRefresh = async () => {
    setRefreshing(true)
    try {
      await Stores.HubCatalog.refresh()
      // The refresh handler returns an updated/new_version tuple,
      // but the user just needs a success toast.
      // Read via `$` snapshot (not the render-only `Stores.HubCatalog.*`
      // reactive read, which calls a hook — illegal inside this async handler
      // and throws React #321, swallowing the success toast).
      message.success(`Hub catalog refreshed to v${Stores.HubCatalog.$.hubVersion ?? '?'}`)
      // Trigger each visible tab's own refresh hook so per-tab lists
      // re-render against the new catalog (the back-compat per-category
      // endpoints already serve from the rotated `current/` dir).
      for (const tab of visibleTabs) {
        try {
          await tab.refresh()
        } catch (e) {
          console.warn(`hub tab ${tab.id} refresh failed:`, e)
        }
      }
    } catch (error) {
      message.error(
        `Failed to refresh hub catalog: ${(error as Error)?.message ?? error}`,
      )
      console.error(error)
    } finally {
      setRefreshing(false)
    }
  }

  const handleTabClick = (key: string) => navigate(`/hub/${key}`)

  // Left side-menu items (desktop).
  const kitMenuItems: MenuItem[] = visibleTabs.map(tab => ({
    key: tab.id,
    icon: tab.icon,
    label: tab.label,
  }))

  // Dropdown items for the mobile header.
  const dropdownItems = visibleTabs.map(tab => ({
    key: tab.id,
    label: (
      <Flex className="gap-2 items-center">
        {tab.icon}
        {tab.label}
      </Flex>
    ),
  }))

  const currentTabLabel = currentTabSlot ? (
    <Flex align="center" className="gap-1">
      {currentTabSlot.icon}
      {currentTabSlot.label}
    </Flex>
  ) : null

  // Catalog-version indicator — read-only build marker, shown in the header.
  const versionTag = hubVersion ? (
    <Tooltip
      content={
        serverVersion
          ? `Server v${serverVersion} — installed catalog from ziee-ai/hub`
          : 'Installed catalog from ziee-ai/hub'
      }
    >
      <Tag data-testid="hub-version-tag">v{hubVersion}</Tag>
    </Tooltip>
  ) : null

  const HubMenu = () => (
    <Menu
      data-testid="hub-nav-menu"
      className="w-fit px-2 py-1"
      items={kitMenuItems}
      selectedKey={activeTab}
      onSelect={handleTabClick}
      mode="vertical"
      aria-label="Hub sections"
    />
  )

  return (
    <div
      ref={containerRef}
      className={cn(
        'flex flex-col w-full',
        nativeScroll ? 'min-h-dvh' : 'h-full overflow-hidden',
      )}
    >
      {/* Page Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full gap-2">
          <Title level={4} className="!m-0 !leading-tight shrink-0">
            Hub
          </Title>
          <Flex align="center" className="gap-2 min-w-0">
            {/* The version tag is a non-critical build marker (its info is in the
                tooltip); on the narrowest widths it yields so the title + section
                dropdown stay fully legible instead of the title crushing to "H…". */}
            {!minSize.xs && versionTag}
            {/* Mobile: refresh icon + the section dropdown (the left menu is
                hidden). Desktop keeps both on the left side-menu instead. */}
            {useMobileLayout && (
              <>
                {canRefresh && (
                  <Button
                    icon={<RotateCw />}
                    onClick={handleRefresh}
                    loading={refreshing}
                    variant="ghost"
                    size="icon"
                    tooltip="Refresh catalog"
                    data-testid="hub-refresh-btn"
                  />
                )}
                <Dropdown
                  data-testid="hub-tabs-dropdown"
                  items={dropdownItems}
                  onSelect={handleTabClick}
                  onOpenChange={setMobileMenuOpen}
                >
                  <Button
                    variant="ghost"
                    data-testid="hub-tabs-dropdown-btn"
                    aria-label="Select hub section"
                    aria-haspopup="menu"
                    aria-expanded={mobileMenuOpen}
                  >
                    {currentTabLabel} <IoIosArrowDown />
                  </Button>
                </Dropdown>
              </>
            )}
          </Flex>
        </div>
      </HeaderBarContainer>

      {/* Page Content */}
      <div className={cn('flex flex-1', nativeScroll ? '' : 'overflow-hidden')}>
        {/* Desktop left side-menu — sections + a pinned Refresh at the bottom
            (mirrors the Settings page's nav + onboarding/help footer). */}
        {!useMobileLayout && (
          <div
            className="w-fit pt-1 flex flex-col h-full"
            role="navigation"
            aria-label="Hub sections"
          >
            <ScrollArea axis="y" className="flex-1 min-h-0">
              <HubMenu />
            </ScrollArea>
            {canRefresh && (
              <div className="border-t border-border p-2 flex flex-col gap-1">
                <Button
                  data-testid="hub-refresh-btn"
                  variant="ghost"
                  size="default"
                  icon={<RotateCw />}
                  loading={refreshing}
                  className="justify-start"
                  onClick={handleRefresh}
                >
                  Refresh
                </Button>
              </div>
            )}
          </div>
        )}

        {/* Main Content Area */}
        <div className={cn('flex-1', nativeScroll ? '' : 'overflow-hidden')}>
          {urlSegmentIsForbidden ? (
            <Result
              data-testid="hub-forbidden-result"
              status="403"
              title="Not authorized"
              subtitle="You don't have permission to view this Hub tab."
            />
          ) : (
            <DivScrollY
              nativeFlow
              className={cn(
                'flex flex-1 w-full flex-col',
                nativeScroll ? '' : 'h-full overflow-y-auto',
              )}
            >
              <div className="max-w-4xl w-full flex flex-col self-center">
                <div
                  className="flex flex-col py-3 w-full"
                  style={
                    nativeScroll
                      ? {
                          paddingBottom:
                            'calc(env(safe-area-inset-bottom, 0px) + 12px)',
                        }
                      : undefined
                  }
                >
                  {currentTabSlot && (
                    <LazyComponentRenderer
                      component={currentTabSlot.component}
                      fallback={<div>Loading...</div>}
                    />
                  )}
                </div>
              </div>
            </DivScrollY>
          )}
        </div>
      </div>
    </div>
  )
}

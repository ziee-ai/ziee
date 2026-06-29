import { useState, useMemo, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import {
  Button,
  Dropdown,
  Flex,
  Result,
  Segmented,
  Tag,
  Tooltip,
  Text,
  message,
} from '@/components/ui'
import { RotateCw } from 'lucide-react'
import { IoIosArrowDown, IoIosArrowForward } from 'react-icons/io'
import { Stores } from '@/core/stores'
import { evaluatePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { DivScrollY } from '@/components/common/DivScrollY'

export function HubPage() {
  const { activeTab: urlActiveTab } = useParams()
  const navigate = useNavigate()
  const { slots } = Stores.ModuleSystem
  const { user, permissions } = Stores.Auth
  // Subscribe to the MCP policy so the MCP tab's shouldRender gate
  // re-evaluates the moment an admin saves a new policy. The
  // `mcpPolicy` value itself isn't used here — its presence in the
  // visibleTabs useMemo deps below is the load-bearing piece.
  const { policy: mcpPolicy } = Stores.McpUserPolicy
  const windowMinSize = useWindowMinSize()
  const [refreshing, setRefreshing] = useState(false)

  // Get hub tabs from slot system, sorted
  const hubTabs = useMemo(() => {
    return (slots.get('hubTabs') || []).sort((a, b) => a.order - b.order)
  }, [slots])

  // Filter to tabs the current user has read permission on AND
  // whose optional `shouldRender` gate (admin policy / runtime
  // config) returns true. `shouldRender` is omitted on most tabs;
  // when present, evaluated alongside the permission check.
  // `mcpPolicy` in deps is what makes the MCP tab re-evaluate when
  // the admin policy changes (its shouldRender calls into the
  // policy store).
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

  // Redirect to first visible tab if at /hub with no segment. Skip the
  // redirect when the user has no visible tabs OR when they've
  // explicitly navigated to a tab they don't have access to — in both
  // cases we render an inline 403 instead.
  const hasUrlSegment = !!urlActiveTab
  const urlSegmentIsRegistered =
    hasUrlSegment && hubTabs.some(t => t.id === urlActiveTab)
  const urlSegmentIsForbidden =
    urlSegmentIsRegistered &&
    !visibleTabs.some(t => t.id === urlActiveTab)

  useEffect(() => {
    if (
      !hasUrlSegment &&
      visibleTabs.length > 0 &&
      !urlSegmentIsForbidden
    ) {
      navigate(`/hub/${visibleTabs[0].id}`, { replace: true })
    }
  }, [hasUrlSegment, visibleTabs, navigate, urlSegmentIsForbidden])

  const currentTabSlot = visibleTabs.find(t => t.id === activeTab)
  // Page-level Refresh is admin-only now (calls unified
  // POST /api/hub/refresh — fetches the latest signed catalog from
  // GitHub, sha256 + cosign verifies, atomic rotate). Per-tab refresh
  // hooks are still defined on the slot for backwards-compat with the
  // legacy event-bus surface, but the button no longer dispatches
  // through them.
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
      message.success(`Hub catalog refreshed to v${Stores.HubCatalog.hubVersion ?? '?'}`)
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
      message.error(`Failed to refresh hub catalog: ${(error as Error)?.message ?? error}`)
      console.error(error)
    } finally {
      setRefreshing(false)
    }
  }

  // Segmented options for desktop
  const segmentedOptions = visibleTabs.map(tab => ({
    value: tab.id,
    label: (
      <Flex align="center" className="gap-1">
        {tab.icon}
        {tab.label}
      </Flex>
    ),
  }))

  // Dropdown items for mobile
  const dropdownItems = visibleTabs.map(tab => ({
    key: tab.id,
    label: (
      <Flex className="gap-2">
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

  return (
    <Flex className="flex flex-col w-full h-full overflow-hidden">
      <HeaderBarContainer>
        <div className="flex items-center justify-between w-full h-[50px]">
          <Text ellipsis className="!m-0 !leading-tight">
            Hub
          </Text>

          {/* Desktop: Show segmented control in title bar center */}
          {!windowMinSize.xs && (
            <div className="flex-1 flex h-full justify-center items-center">
              <Segmented
                data-testid="hub-tabs-segmented"
                value={activeTab}
                onChange={(value: string) => {
                  navigate(`/hub/${value}`)
                }}
                options={segmentedOptions}
              />
            </div>
          )}

          {/* Mobile: Show dropdown in title bar */}
          {windowMinSize.xs && (
            <div className="flex flex-1 items-center px-2">
              <IoIosArrowForward />
              <Dropdown
                data-testid="hub-tabs-dropdown"
                items={dropdownItems}
                onSelect={(key: string) => {
                  navigate(`/hub/${key}`)
                }}
              >
                <Button variant="ghost" className="!pt-1" data-testid="hub-tabs-dropdown-btn">
                  {currentTabLabel} <IoIosArrowDown />
                </Button>
              </Dropdown>
            </div>
          )}

          <Flex align="center" className="gap-2">
            {/* Catalog-version indicator. Hub v2 uses per-entry semver;
                the catalog hub_version is now just a build marker shown
                read-only here for diagnostics (and is identical for
                admins + users — no version picker anymore). */}
            {hubVersion && (
              <Tooltip
                content={
                  serverVersion
                    ? `Server v${serverVersion} — installed catalog from ziee-ai/hub`
                    : 'Installed catalog from ziee-ai/hub'
                }
              >
                <Tag data-testid="hub-version-tag">v{hubVersion}</Tag>
              </Tooltip>
            )}
            {canRefresh && (
              <Button
                icon={<RotateCw />}
                onClick={handleRefresh}
                loading={refreshing}
                variant="ghost"
                data-testid="hub-refresh-btn"
              >
                {windowMinSize.xs ? null : 'Refresh'}
              </Button>
            )}
          </Flex>
        </div>
      </HeaderBarContainer>

      <div className="flex flex-col w-full h-full overflow-hidden">
        <DivScrollY className="flex flex-1 w-full flex-col overflow-y-auto">
          <div className="max-w-4xl w-full flex flex-col self-center">
            <div className="flex-1 h-full w-full overflow-y-auto">
              <div className="flex flex-col py-3 w-full">
                {urlSegmentIsForbidden ? (
                  <Result
                    data-testid="hub-forbidden-result"
                    status="403"
                    title="Not authorized"
                    subtitle="You don't have permission to view this Hub tab."
                  />
                ) : (
                  currentTabSlot && (
                    <LazyComponentRenderer
                      component={currentTabSlot.component}
                      fallback={<div>Loading...</div>}
                    />
                  )
                )}
              </div>
            </div>
          </div>
        </DivScrollY>
      </div>
    </Flex>
  )
}

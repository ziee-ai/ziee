import { useState, useMemo, useEffect } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { Button, Dropdown, Flex, Result, Segmented, theme, Typography } from 'antd'
import { ReloadOutlined } from '@ant-design/icons'
import { IoIosArrowDown, IoIosArrowForward } from 'react-icons/io'
import { Stores } from '@/core/stores'
import { evaluatePermission } from '@/core/permissions'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { message } from 'antd'
import { DivScrollY } from '@/components/common/DivScrollY'

export function HubPage() {
  const { activeTab: urlActiveTab } = useParams()
  const navigate = useNavigate()
  const { slots } = Stores.ModuleSystem
  const { user, permissions } = Stores.Auth
  const windowMinSize = useWindowMinSize()
  const { token } = theme.useToken()
  const [refreshing, setRefreshing] = useState(false)

  // Get hub tabs from slot system, sorted
  const hubTabs = useMemo(() => {
    return (slots.get('hubTabs') || []).sort((a, b) => a.order - b.order)
  }, [slots])

  // Filter to tabs the current user has read permission on
  const visibleTabs = useMemo(
    () =>
      hubTabs.filter(t =>
        evaluatePermission(user, permissions, t.permissions.read),
      ),
    [hubTabs, user, permissions],
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
  const canRefresh = currentTabSlot?.permissions.refresh
    ? evaluatePermission(user, permissions, currentTabSlot.permissions.refresh)
    : true

  const handleRefresh = async () => {
    setRefreshing(true)
    try {
      if (currentTabSlot?.refresh) {
        await currentTabSlot.refresh()
        message.success('Hub data refreshed successfully')
      }
    } catch (error) {
      message.error('Failed to refresh hub data')
      console.error(error)
    } finally {
      setRefreshing(false)
    }
  }

  // Segmented options for desktop
  const segmentedOptions = visibleTabs.map(tab => ({
    value: tab.id,
    label: (
      <Flex align="center" gap={4}>
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
    <Flex align="center" gap={4}>
      {currentTabSlot.icon}
      {currentTabSlot.label}
    </Flex>
  ) : null

  return (
    <Flex className="flex flex-col w-full h-full overflow-hidden">
      <HeaderBarContainer>
        <div className="flex items-center justify-between w-full h-[50px]">
          <Typography.Title level={3} ellipsis className="!m-0 !leading-tight">
            Hub
          </Typography.Title>

          {/* Desktop: Show segmented control in title bar center */}
          {!windowMinSize.xs && (
            <div className="flex-1 flex h-full justify-center items-center">
              <Segmented
                value={activeTab}
                onChange={(value: string) => {
                  navigate(`/hub/${value}`)
                }}
                className="[&_.ant-segmented-item-label]:!px-4 [&_.ant-segmented-item-label]:!py-1"
                style={{
                  backgroundColor: token.colorBgMask,
                }}
                shape="round"
                options={segmentedOptions}
              />
            </div>
          )}

          {/* Mobile: Show dropdown in title bar */}
          {windowMinSize.xs && (
            <div className="flex flex-1 items-center px-2">
              <IoIosArrowForward />
              <Dropdown
                menu={{
                  items: dropdownItems,
                  onClick: ({ key }) => {
                    navigate(`/hub/${key}`)
                  },
                  selectedKeys: [activeTab],
                }}
                trigger={['click']}
              >
                <Button type="text" className="!pt-1">
                  {currentTabLabel} <IoIosArrowDown />
                </Button>
              </Dropdown>
            </div>
          )}

          {canRefresh && (
            <Button
              icon={<ReloadOutlined />}
              onClick={handleRefresh}
              loading={refreshing}
              type="text"
            >
              {windowMinSize.xs ? null : 'Refresh'}
            </Button>
          )}
        </div>
      </HeaderBarContainer>

      <div className="flex flex-col w-full h-full overflow-hidden">
        <DivScrollY className="flex flex-1 w-full flex-col overflow-y-auto">
          <div className="max-w-4xl w-full flex flex-col self-center">
            <div className="flex-1 h-full w-full overflow-y-auto">
              <div className="flex flex-col py-3 w-full">
                {urlSegmentIsForbidden ? (
                  <Result
                    status="403"
                    title="Not authorized"
                    subTitle="You don't have permission to view this Hub tab."
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

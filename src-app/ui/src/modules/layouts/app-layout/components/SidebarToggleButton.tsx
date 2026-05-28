import { Button, Tooltip } from 'antd'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'

export function SidebarToggleButton() {
  const { isSidebarCollapsed } = Stores.AppLayout

  // Single compact size at every breakpoint. The previous
  // responsive flip (44px on xs viewports for WCAG 2.5.5 touch
  // target) caused a jarring "the button looks weird / out of
  // place" jump when the user resized a desktop browser narrow —
  // and on actual touch devices the icon still gets the full
  // hit slop of the surrounding fixed-positioned wrapper.
  const dimension = '28px'
  const iconFontSize = '20px'

  return (
    <div
      className="flex items-center gap-6 mr-4 fixed z-10 h-[50px]"
      style={{
        left: 12,
        top: 0,
      }}
    >
      <Tooltip
        title={isSidebarCollapsed ? 'Open sidebar' : 'Close sidebar'}
        placement="right"
      >
        <Button
          type="text"
          onClick={Stores.AppLayout.toggleSidebar}
          className="flex items-center justify-center"
          style={{
            width: dimension,
            height: dimension,
            padding: 0,
            fontSize: iconFontSize,
            borderRadius: '4px',
            minWidth: dimension,
          }}
          aria-label={isSidebarCollapsed ? 'Open navigation menu' : 'Close navigation menu'}
          aria-expanded={!isSidebarCollapsed}
          aria-controls="app-sidebar"
        >
          {isSidebarCollapsed ? (
            <GoSidebarCollapse aria-hidden="true" />
          ) : (
            <GoSidebarExpand aria-hidden="true" />
          )}
        </Button>
      </Tooltip>
    </div>
  )
}

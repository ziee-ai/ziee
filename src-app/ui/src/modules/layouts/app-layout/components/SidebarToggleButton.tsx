import { Tooltip, Button } from '@/components/ui'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'

export function SidebarToggleButton() {
  const { isSidebarCollapsed, nativeScroll, headerHidden } = Stores.AppLayout

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
      className={cn(
        'flex items-center gap-6 mr-4 fixed',
        // Above the mobile sidebar Sheet (z-50) so the toggle stays on top of
        // the open sidebar (and above the z-30 header); below Dialog (z-60).
        nativeScroll ? 'h-[40px] z-[55]' : 'h-[50px] z-10',
      )}
      style={{
        left: 12,
        // Native (mobile Settings): match the header exactly — safe-area (clear
        // the notch) + the header's 5px offset — so the button's center lines up
        // with the header text and the top band stays 50px.
        top: nativeScroll ? 'calc(env(safe-area-inset-top, 0px) + 5px)' : 0,
        // Disappear/reappear together with the auto-hiding header.
        ...(nativeScroll
          ? {
              transform: headerHidden ? 'translateY(-150%)' : 'translateY(0)',
              opacity: headerHidden ? 0 : 1,
              pointerEvents: headerHidden ? 'none' : 'auto',
              transition: 'transform 0.3s ease, opacity 0.3s ease',
            }
          : null),
      }}
    >
      <Tooltip
        title={isSidebarCollapsed ? 'Open sidebar' : 'Close sidebar'}
        side="right"
      >
        <Button
          variant="ghost"
          data-testid="layout-sidebar-toggle-button"
          onClick={() => Stores.AppLayout.toggleSidebar()}
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

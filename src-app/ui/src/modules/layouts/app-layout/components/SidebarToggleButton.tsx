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
        // z-index depends on whether the left sidebar Sheet is open:
        //  • sidebar OPEN (!collapsed → Sheet showing, z-50): sit ABOVE it
        //    (z-55) so the toggle stays reachable to close the sidebar.
        //  • sidebar COLLAPSED (Sheet closed): drop to z-35 — still above the
        //    z-30 header, but BELOW a right Drawer's z-40 scrim, so an open
        //    right Drawer paints over the toggle. The two states are mutually
        //    exclusive (a right Drawer only opens while the sidebar is
        //    collapsed), so this single flag covers both without tracking
        //    drawer state.
        nativeScroll
          ? cn('h-[40px]', isSidebarCollapsed ? 'z-[35]' : 'z-[55]')
          : 'h-[50px] z-10',
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
              // Match the header bar's show animation (duration-300 ease-out) so
              // the toggle and the header arrive/leave in sync rather than on two
              // different easings.
              transition: 'transform 0.3s ease-out, opacity 0.3s ease-out',
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

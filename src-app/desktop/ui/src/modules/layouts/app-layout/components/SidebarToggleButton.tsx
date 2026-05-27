/**
 * DELIBERATE DIVERGENCE from core's SidebarToggleButton.
 *
 * Inherits from core:
 *   - useWindowMinSize() responsive sizing (44px on mobile per
 *     WCAG 2.5.5 Target Size; compact 24px on desktop).
 *   - Full ARIA wiring: aria-label, aria-expanded, aria-controls.
 *
 * Desktop-only additions:
 *   - <TauriDragRegion> covering the top strip so the user can
 *     drag-move the window from the empty space around the button.
 *   - macOS-only `marginLeft: 76px` shift so the button doesn't sit
 *     under the traffic-light controls. Cleared in fullscreen mode
 *     (traffic lights vanish then).
 *
 * Keep in sync with `ui/src/modules/layouts/app-layout/components/SidebarToggleButton.tsx`;
 * `just desktop-drift-check` flags any divergence.
 */

import { Button } from 'antd'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { TauriDragRegion } from '@ziee/desktop/components/TauriDragRegion.tsx'

export function SidebarToggleButton() {
  const { isSidebarCollapsed, isFullscreen } = Stores.AppLayout
  const windowMinSize = useWindowMinSize()

  // Mobile (≤480px viewport): 44×44 tap target per WCAG 2.5.5.
  // Desktop: compact 24px chevron. (core: audit 02 R-1.)
  const isMobile = windowMinSize.xs
  const dimension = isMobile ? '44px' : '24px'
  const iconFontSize = isMobile ? '24px' : '30px'

  // macOS Tauri window has traffic-light controls in the top-left ~70px;
  // clear them by shifting the toggle button right. Vanish in fullscreen.
  const macTrafficLightOffset =
    isTauriView && isMacOS && !isFullscreen ? 76 : 12

  return (
    <>
      <TauriDragRegion
        className={'gap-6 fixed z-1 h-[50px] top-0 left-0 w-full'}
      />
      <div className="flex items-center gap-6 fixed z-10 h-[50px] top-0">
        <Button
          type="text"
          onClick={Stores.AppLayout.toggleSidebar}
          className="flex items-center justify-center"
          style={{
            marginLeft: macTrafficLightOffset,
            width: dimension,
            height: dimension,
            padding: 0,
            fontSize: iconFontSize,
            borderRadius: '4px',
            minWidth: dimension,
          }}
          aria-label={
            isSidebarCollapsed
              ? 'Open navigation menu'
              : 'Close navigation menu'
          }
          aria-expanded={!isSidebarCollapsed}
          aria-controls="app-sidebar"
        >
          {isSidebarCollapsed ? (
            <GoSidebarCollapse aria-hidden="true" />
          ) : (
            <GoSidebarExpand aria-hidden="true" />
          )}
        </Button>
      </div>
    </>
  )
}

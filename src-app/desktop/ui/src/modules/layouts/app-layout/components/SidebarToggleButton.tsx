/**
 * Desktop Override: SidebarToggleButton
 *
 * - Parent div stretches full width and is draggable
 * - Button has left padding on macOS Tauri view to avoid traffic lights
 */

import { Button } from 'antd'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'
import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { TauriDragRegion } from '@ziee/desktop/components/TauriDragRegion.tsx'

export function SidebarToggleButton() {
  const { isSidebarCollapsed, isFullscreen } = Stores.AppLayout

  // Add left padding for macOS traffic lights
  const buttonLeftPosition = isTauriView && isMacOS && !isFullscreen ? 76 : 12

  return (
    <>
      <TauriDragRegion
        className={'gap-6 fixed z-1 h-[50px] top-0 left-0 w-full'}
      />
      <div className="flex items-center gap-6 fixed z-10 h-[50px]">
        <Button
          type="text"
          onClick={Stores.AppLayout.toggleSidebar}
          className="flex items-center justify-center"
          style={{
            marginLeft: buttonLeftPosition,
            width: '24px',
            height: '24px',
            padding: 0,
            fontSize: '30px',
            borderRadius: '4px',
            minWidth: '20px',
          }}
          aria-label={
            isSidebarCollapsed ? 'Expand sidebar' : 'Collapse sidebar'
          }
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

/**
 * DELIBERATE DIVERGENCE from core's SidebarToggleButton.
 *
 * Differs from core:
 *   - Single 28px button at every breakpoint (core uses 44px on
 *     ≤480px for WCAG-2.5.5 touch targets). Tauri desktop is always
 *     mouse/trackpad; the responsive flip caused a jarring size
 *     change when resizing the window across the xs threshold.
 *   - macOS-only `marginLeft: 76px` shift so the button clears the
 *     traffic-light controls. Cleared in fullscreen mode.
 *   - <TauriDragRegion> overlay covering the top strip so the
 *     surrounding empty area drags the window.
 *
 * Inherits from core:
 *   - Full ARIA wiring: aria-label, aria-expanded, aria-controls.
 */

import { Button, Tooltip } from 'antd'
import { GoSidebarCollapse, GoSidebarExpand } from 'react-icons/go'
import { Stores } from '@/core/stores'
import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { TauriDragRegion } from '@ziee/desktop/components/TauriDragRegion.tsx'

export function SidebarToggleButton() {
  const { isSidebarCollapsed, isFullscreen } = Stores.AppLayout

  // Tauri desktop is always mouse/trackpad — the WCAG-2.5.5 44px
  // touch target the core uses isn't required here. Keep a single
  // compact size so resizing the window across the xs threshold
  // doesn't morph the chevron (28px button at every breakpoint,
  // 20px icon that fits inside it cleanly — prior 30px icon
  // overflowed the 24px button and showed an oversized hover bg).
  const dimension = '28px'
  const iconFontSize = '20px'

  // macOS Tauri traffic lights now start at x=20 (per
  // `backend/mod.rs`'s `traffic_light_position`), cluster width ~52px,
  // so they end around x=72. Shift the toggle right to x=84 — 12px
  // gap matches the spacing other macOS apps leave between the
  // traffic lights and the first toolbar control. Vanish in
  // fullscreen (no traffic lights).
  const macTrafficLightOffset =
    isTauriView && isMacOS && !isFullscreen ? 84 : 12

  // Full-width top-strip drag overlay (z:1) so pages WITHOUT a
  // HeaderBarContainer (NewChatPage etc.) and the sidebar's top
  // 50px both remain draggable. HeaderBarContainer raises its own
  // stacking level (`position: relative; z-index: 2`) so its
  // content paints above this overlay and its per-component
  // manual mousedown handler takes over there — that's how header
  // buttons stay clickable while the rest of the top strip drags.
  return (
    <>
      <TauriDragRegion
        className={'gap-6 fixed z-1 h-[50px] top-0 left-0 w-full'}
      />
      <div className="flex items-center gap-6 fixed z-10 h-[50px] top-0">
        <Tooltip
          title={isSidebarCollapsed ? 'Open sidebar' : 'Close sidebar'}
          placement="right"
        >
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
        </Tooltip>
      </div>
    </>
  )
}

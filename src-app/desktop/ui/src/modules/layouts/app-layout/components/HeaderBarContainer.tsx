/**
 * DELIBERATE DIVERGENCE from core's HeaderBarContainer.
 *
 * Core simply renders a 50px div with `padding-left: 48 | 12` and
 * `padding-right: 12`. Desktop adds:
 *
 *   - <TauriDragRegion> overlay so the user can drag the window from
 *     the empty area around the header content.
 *   - Larger `padding-left` (110px on macOS Tauri, sidebar collapsed,
 *     not fullscreen) to clear the traffic-light controls.
 *   - Larger `padding-right` (100px on Windows/Linux Tauri, not
 *     fullscreen) to clear minimize/maximize/close window controls.
 *   - Debounced ref-based style writes (`useLayoutEffect` with a
 *     setTimeout(0) coalesce) so the multi-pass renders that fire
 *     during sidebar-collapse animations don't thrash the DOM.
 *
 * If core grows new layout primitives, port them into the inline
 * `style` object below or the className.
 */

import { useRef, useLayoutEffect } from 'react'
import { theme } from 'antd'
import { Stores } from '@/core/stores'
import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { TauriDragRegion } from '@ziee/desktop/components/TauriDragRegion'

interface HeaderBarContainerProps {
  children?: React.ReactNode
  className?: string
  style?: React.CSSProperties
}

export const HeaderBarContainer = ({
  children,
  className = '',
  style = {},
}: HeaderBarContainerProps) => {
  const { token } = theme.useToken()
  const { isSidebarCollapsed, isFullscreen } = Stores.AppLayout
  const containerRef = useRef<HTMLDivElement>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  )

  // macOS Tauri: ~110px on the left when the sidebar is collapsed so the
  // header content doesn't sit under the traffic-light controls. Off in
  // fullscreen (no traffic lights). Web / non-Tauri: core's 48 | 12.
  const paddingLeft =
    isSidebarCollapsed && isTauriView && !isFullscreen && isMacOS
      ? 110
      : isSidebarCollapsed
        ? 48
        : 12

  // Windows/Linux Tauri: ~100px on the right to clear the minimize /
  // maximize / close trio. Off in fullscreen. Web: 12.
  const paddingRight = isTauriView && !isFullscreen && !isMacOS ? 100 : 12

  // Coalesced style write — sidebar-collapse animates over ~200ms and
  // triggers many renders; writing padding inline on every render would
  // re-trigger layout. The setTimeout-0 trick lets React's batch
  // settle, then we apply once.
  useLayoutEffect(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }
    timeoutRef.current = setTimeout(() => {
      if (containerRef.current) {
        containerRef.current.style.paddingLeft = `${paddingLeft}px`
        containerRef.current.style.paddingRight = `${paddingRight}px`
      }
    }, 0)

    return () => {
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current)
      }
    }
  }, [paddingLeft, paddingRight])

  return (
    <div
      ref={containerRef}
      className={`h-[50px] w-full flex relative border-b transition-[padding] duration-200 ease-in-out box-border ${className}`}
      style={{
        borderColor: token.colorBorderSecondary,
        ...style,
      }}
    >
      {/* Drag region overlay - absolute positioned behind content */}
      <TauriDragRegion className="h-full w-full absolute top-0 left-0" />
      {children}
    </div>
  )
}

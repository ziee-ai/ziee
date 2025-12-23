/**
 * Desktop Override: HeaderBarContainer
 *
 * Adds Tauri drag region overlay and adjusts padding for window controls:
 * - macOS: Extra left padding for traffic lights when sidebar collapsed
 * - Windows/Linux: Extra right padding for window controls
 *
 * Uses debounced ref-based style updates to handle multiple React re-renders smoothly
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
  const timeoutRef = useRef<ReturnType<typeof setTimeout>>()

  // Calculate left padding
  // macOS: need extra space for traffic lights when sidebar is collapsed
  const paddingLeft =
    isSidebarCollapsed && isTauriView && !isFullscreen && isMacOS
      ? 110
      : isSidebarCollapsed
        ? 48
        : 12

  // Calculate right padding
  // Windows/Linux: need extra space for window controls
  const paddingRight = isTauriView && !isFullscreen && !isMacOS ? 100 : 12

  // Debounced style update - waits for renders to settle before updating DOM
  useLayoutEffect(() => {
    if (timeoutRef.current) {
      clearTimeout(timeoutRef.current)
    }
    timeoutRef.current = setTimeout(() => {
      if (containerRef.current) {
        console.log('Updating HeaderBarContainer padding:')
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

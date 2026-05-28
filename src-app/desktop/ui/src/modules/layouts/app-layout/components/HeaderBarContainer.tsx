/**
 * DELIBERATE DIVERGENCE from core's HeaderBarContainer.
 *
 * Core simply renders a 50px div with `padding-left: 48 | 12` and
 * `padding-right: 12`. Desktop adds:
 *
 *   - Manual window-drag via `getCurrentWindow().startDragging()`
 *     bound to mousedown, with an interactive-target exemption
 *     (Buttons, Selects, Dropdowns, etc.). Tauri's automatic
 *     `data-tauri-drag-region` attribute is intentionally NOT used:
 *     it inspects only the immediate event target, but header
 *     content (TitleEditor, project chip slot, etc.) typically
 *     uses `w-full max-w-4xl mx-auto` wrappers that cover the
 *     whole row — Tauri never sees the bare header div, so
 *     neither container-attr nor below-z-index-overlay tricks
 *     worked. Manual API call sidesteps the policy entirely.
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

import { useRef, useLayoutEffect, useCallback } from 'react'
import { theme } from 'antd'
import { Stores } from '@/core/stores'
import { isTauriView, isMacOS } from '@ziee/desktop/core/platform'
import { getCurrentWindow } from '@tauri-apps/api/window'

// Selector matching anything we'd consider an "interactive" descendant.
// A mousedown on any of these (or their inner content) should NOT
// initiate a window drag — the click belongs to the control.
const INTERACTIVE_SEL =
  'button, a, input, textarea, select, [role="button"], [role="link"], [role="menuitem"], [role="combobox"], [contenteditable="true"], .ant-select, .ant-dropdown-trigger'

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

  // Manual drag handling via the Tauri API instead of the
  // automatic `data-tauri-drag-region` attribute. Reasoning:
  // Tauri's auto-detection inspects the IMMEDIATE event target;
  // header content (TitleEditor wrapper, project chip slot, etc.)
  // typically uses `w-full max-w-4xl mx-auto` which covers the
  // entire row width, so the target is never the bare header div
  // — neither container-attr nor below-z-index overlay approaches
  // give Tauri a moment to see the attribute. By binding mousedown
  // here we control the policy: ignore clicks on interactive
  // descendants (real buttons/links/inputs etc.), drag on
  // everything else. Plain text wrappers and empty header space
  // both initiate drag.
  const handleHeaderMouseDown = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isTauriView) return
      if (e.button !== 0) return // left-click only
      const target = e.target as Element
      if (target.closest?.(INTERACTIVE_SEL)) return
      e.preventDefault()
      void getCurrentWindow().startDragging()
    },
    [],
  )

  // Standard macOS gesture: double-click the titlebar to
  // maximize / restore. Same interactive-target exemption.
  const handleHeaderDoubleClick = useCallback(
    (e: React.MouseEvent<HTMLDivElement>) => {
      if (!isTauriView) return
      const target = e.target as Element
      if (target.closest?.(INTERACTIVE_SEL)) return
      void getCurrentWindow().toggleMaximize()
    },
    [],
  )

  return (
    <div
      ref={containerRef}
      // Easing must match AppLayout's sidebar transition
      // (`width 200ms ease-out`); a different curve here makes the
      // title text appear to "catch up" after the sidebar has
      // already settled, which reads as lag.
      className={`h-[50px] w-full flex relative border-b transition-[padding] duration-200 ease-out box-border ${className}`}
      style={{
        borderColor: token.colorBorderSecondary,
        // Paint above SidebarToggleButton's full-width drag overlay
        // (fixed z:1) so header content captures pointer events
        // first. The manual mousedown handler below then either
        // bails (interactive target → click fires) or starts drag
        // (empty bg → window-drag).
        zIndex: 2,
        ...style,
      }}
      onMouseDown={handleHeaderMouseDown}
      onDoubleClick={handleHeaderDoubleClick}
    >
      {children}
    </div>
  )
}

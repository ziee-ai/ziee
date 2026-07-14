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
import { Stores } from '@ziee/framework/stores'
import { isTauriView, isMacOS, isLinux } from '@ziee/desktop/core/platform'
import { getCurrentWindow } from '@tauri-apps/api/window'

// Selector matching anything we'd consider an "interactive" descendant.
// A mousedown on any of these (or their inner content) should NOT
// initiate a window drag — the click belongs to the control.
const INTERACTIVE_SEL =
  // antd's <Segmented> renders `<label class="ant-segmented-item">` with a
  // hidden <input type="radio"> inside; the visible label div doesn't
  // match `input` via closest(), so we add `.ant-segmented-item` (and the
  // wrapping `.ant-segmented`) explicitly. The HubPage tabs sit inside
  // HeaderBarContainer — without this exemption, mousedown initiates a
  // window drag and the tab-change click never fires.
  'button, a, input, textarea, select, [role="button"], [role="link"], [role="menuitem"], [role="combobox"], [contenteditable="true"]'

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
  const { isSidebarCollapsed, isFullscreen } = Stores.AppLayout

  // Soft-fade overlay color matched to the content surface, faded
  // through alpha so the gradient doesn't pass through a faint
  // gray midpoint on light themes (which is what the CSS
  // `transparent` keyword would produce). Relative-color syntax keeps
  // the `--card` hue at alpha 0 (was tinycolor over antd's token).
  const fadeOut = 'rgb(from var(--card) r g b / 0)'
  const containerRef = useRef<HTMLDivElement>(null)
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | undefined>(
    undefined,
  )

  // macOS Tauri: ~118px on the left when the sidebar is collapsed so
  // the header content clears BOTH the traffic-light cluster
  // (ends ~x=72 after the x=20 shift) AND the toggle button
  // (marginLeft=84, width=28 → right edge ~112), with ~6px breathing
  // room past that. Off in fullscreen (no traffic lights). Web /
  // non-Tauri: core's 48 | 12.
  const paddingLeft =
    isSidebarCollapsed && isTauriView && !isFullscreen && isMacOS
      ? 118
      : isSidebarCollapsed
        ? 48
        : 12

  // Windows Tauri: ~100px on the right to clear decorum's overlay
  // close/min/max trio (drawn INSIDE the webview at top-right).
  // Linux Tauri: native WM chrome — the close/min/max trio lives
  // OUTSIDE the webview (in the WM titlebar above), so no reservation
  // needed. macOS: traffic lights are top-LEFT; right side stays at 12.
  // Off in fullscreen. Web: 12.
  const paddingRight =
    isTauriView && !isFullscreen && !isMacOS && !isLinux ? 100 : 12

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
      // Linux uses native WM decorations — the WM titlebar OUTSIDE the
      // webview already provides window-drag. Bailing here avoids
      // double-handling that confuses xfwm4 / KWin / Mutter (the WM
      // sees the drag start AND tries to handle the same mousedown).
      if (isLinux) return
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
      if (isLinux) return
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
      className={`h-[50px] w-full flex relative transition-[padding] duration-200 ease-out box-border ${className}`}
      style={{
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
      {/* Soft-fade overlay just below the header. Top edge is the
          content surface color, bottom edge is transparent. When
          content scrolls up beneath the (transparent) header, it
          dissolves into the bg color before being clipped. */}
      {/* token-derived gradient (var(--card) → transparent card); theme-aware, not a hardcoded hue, but no single token class expresses a gradient */}
      <div
        aria-hidden="true"
        data-allow-custom-color
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: '100%',
          height: 16,
          pointerEvents: 'none',
          background: `linear-gradient(to bottom, var(--card), ${fadeOut})`,
        }}
      />
    </div>
  )
}

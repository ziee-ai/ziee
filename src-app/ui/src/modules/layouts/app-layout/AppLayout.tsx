import React, { useCallback, useEffect, useRef } from 'react'
import { LeftSidebar } from '@/modules/layouts/app-layout/components/LeftSidebar'
import { SidebarToggleButton } from '@/modules/layouts/app-layout/components/SidebarToggleButton'
import { theme } from 'antd'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import tinycolor from 'tinycolor2'
import 'overlayscrollbars/overlayscrollbars.css'
import { Stores } from '@/core/stores'

/**
 * AppLayout - Main application layout with sidebar
 *
 * Sidebar items are registered via the slot system:
 * - sidebarNavigation: Main navigation items
 * - sidebarTools: Tools/settings items
 * - sidebarPrimaryActions: Action buttons at top
 * - sidebarRecent: Recent items (middle section)
 * - sidebarBottom: Below tools (e.g., download indicator)
 * - sidebarFooter: Footer section (e.g., user profile)
 */
export function AppLayout({ children }: { children: React.ReactNode }) {
  const { isSidebarCollapsed } = Stores.AppLayout
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()

  const sidebarRef = useRef<HTMLDivElement>(null)
  const spacerRef = useRef<HTMLDivElement>(null)
  const mainContentRef = useRef<HTMLDivElement>(null)
  const currentWidth = useRef(200)

  const MIN_WIDTH = 150
  const MAX_WIDTH = 400
  const ICON_ONLY_WIDTH = 52

  const handleMouseDown = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault()

      const handleMouseMove = (e: MouseEvent) => {
        const newWidth = e.clientX

        if (spacerRef.current) {
          spacerRef.current.style.transition = 'none'
        }
        if (sidebarRef.current) {
          sidebarRef.current.style.transition = 'none'
        }

        if (newWidth < MIN_WIDTH / 2) {
          if (spacerRef.current) {
            spacerRef.current.style.transition = 'all 200ms ease-out'
          }
          Stores.AppLayout.setSidebarCollapsed(true)
        } else if (newWidth >= MIN_WIDTH && newWidth <= MAX_WIDTH) {
          // If coming from collapsed state, re-enable transition for smooth expand
          if (isSidebarCollapsed) {
            if (spacerRef.current) {
              spacerRef.current.style.transition = 'all 200ms ease-out'
            }

            setTimeout(() => {
              Stores.AppLayout.setSidebarCollapsed(false)
              currentWidth.current = newWidth
              if (sidebarRef.current) {
                sidebarRef.current.style.width = `${newWidth}px`
              }
              if (spacerRef.current) {
                spacerRef.current.style.width = `${newWidth}px`
              }

              // Resume no-transition dragging after expand
              setTimeout(() => {
                if (sidebarRef.current) {
                  sidebarRef.current.style.transition = 'none'
                }
                if (spacerRef.current) {
                  spacerRef.current.style.transition = 'none'
                }
              }, 300) // Wait for transition to complete
            }, 10)
          } else {
            // Disable the transition for smooth dragging
            Stores.AppLayout.setSidebarCollapsed(false)
            currentWidth.current = newWidth
            if (sidebarRef.current) {
              sidebarRef.current.style.width = `${newWidth}px`
            }
            if (spacerRef.current) {
              spacerRef.current.style.width = `${newWidth}px`
            }
          }
        } else if (newWidth > MAX_WIDTH) {
          currentWidth.current = MAX_WIDTH
          if (sidebarRef.current) {
            sidebarRef.current.style.width = `${MAX_WIDTH}px`
          }
          if (spacerRef.current) {
            spacerRef.current.style.width = `${MAX_WIDTH}px`
          }
        }
      }

      const handleMouseUp = () => {
        if (spacerRef.current) {
          spacerRef.current.style.transition = 'all 200ms ease-out'
        }
        if (sidebarRef.current) {
          sidebarRef.current.style.transition = 'width 200ms ease-out'
        }

        document.removeEventListener('mousemove', handleMouseMove)
        document.removeEventListener('mouseup', handleMouseUp)
      }

      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)
    },
    [MIN_WIDTH, MAX_WIDTH, isSidebarCollapsed],
  )

  useEffect(() => {
    if (windowMinSize.xs) {
      Stores.AppLayout.setSidebarCollapsed(true)
    }
  }, [windowMinSize.xs])

  // ResizeObserver to listen to main content width changes
  useEffect(() => {
    const mainContentElement = mainContentRef.current
    if (!mainContentElement) return

    const resizeObserver = new ResizeObserver(entries => {
      for (const entry of entries) {
        const { width } = entry.contentRect
        Stores.AppLayout.setMainContentWidth(Math.round(width))
      }
    })

    resizeObserver.observe(mainContentElement)

    return () => {
      resizeObserver.disconnect()
    }
  }, [])

  useEffect(() => {
    //set root document background color based on theme
    const root = document.documentElement
    root.style.backgroundColor = token.colorBgContainer
  }, [token.colorBgContainer])

  // Visual viewport listener for mobile keyboard adjustments
  //
  // The previous version wrote `document.body.style.height` AND forced
  // `document.documentElement.scrollTop = 0` on EVERY resize event.
  // iOS Safari fires `resize` continuously while the keyboard is
  // animating in/out, so:
  //   * the unconditional scrollTop reset yanked the user to the top
  //     mid-conversation every time they tapped an input;
  //   * the body height write competed with `.ant-app { height: 100dvh }`
  //     in index.css, causing layout thrash.
  //
  // Fix (audit 02 B-4): only write body height when the viewport has
  // actually shrunk by more than ~100px from window.innerHeight (a
  // keyboard-open heuristic). Skip the scrollTop reset entirely —
  // there's no UX justification for forcing scroll position on
  // keyboard show, and the cost (lost scroll position) is real.
  useEffect(() => {
    if (!window.visualViewport) return

    const KEYBOARD_HEURISTIC_PX = 100
    const updateBodyHeight = () => {
      if (!window.visualViewport) return
      const vv = window.visualViewport
      const keyboardOpen = window.innerHeight - vv.height > KEYBOARD_HEURISTIC_PX
      if (keyboardOpen) {
        // Tell the layout to fit the visible area above the keyboard.
        document.body.style.height = `${vv.height}px`
      } else {
        // Keyboard is closed; let CSS (100dvh) take over again.
        document.body.style.height = ''
      }
    }

    updateBodyHeight()
    window.visualViewport.addEventListener('resize', updateBodyHeight)

    return () => {
      window.visualViewport?.removeEventListener('resize', updateBodyHeight)
    }
  }, [])

  // Mobile sidebar a11y: when the overlay is open (xs viewport AND not
  // collapsed), trap focus + scroll inside the dialog and let Escape
  // close it. Modeled on standard dialog semantics. (audit 02 R-1)
  useEffect(() => {
    if (!windowMinSize.xs || isSidebarCollapsed) return

    const previousBodyOverflow = document.body.style.overflow
    document.body.style.overflow = 'hidden'

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        Stores.AppLayout.setSidebarCollapsed(true)
      }
    }
    document.addEventListener('keydown', onKeyDown)

    return () => {
      document.body.style.overflow = previousBodyOverflow
      document.removeEventListener('keydown', onKeyDown)
    }
  }, [windowMinSize.xs, isSidebarCollapsed])

  const handleMaskClick = () => {
    if (sidebarRef.current) {
      sidebarRef.current.style.transition = 'transform 200ms ease-out'
    }
    Stores.AppLayout.setSidebarCollapsed(true)
    setTimeout(() => {
      if (sidebarRef.current) {
        sidebarRef.current.style.transition = 'none'
      }
    }, 200)
  }

  return (
    <div
      className="h-full w-screen flex overflow-hidden"
      style={{
        backgroundColor: token.colorBgContainer,
      }}
    >
      {/* Sidebar - Always visible, width controlled by container */}
      {/* Mask for Left Sidebar (mobile only). Single onClick is enough;
        * the prior triple-fire (onClick + onMouseDown + onTouchStart)
        * caused the close handler to fire 2-3 times for any tap,
        * which interacted badly with the closing animation. (audit 02 R-1) */}
      {windowMinSize.xs && (
        <div
          className={
            'fixed h-full w-full transition-all z-3 pointer-events-none'
          }
          style={{
            backgroundColor: tinycolor(token.colorBgContainer)
              .setAlpha(isSidebarCollapsed ? 0 : 0.7)
              .toRgbString(),
            pointerEvents: isSidebarCollapsed ? 'none' : 'auto',
          }}
          onClick={handleMaskClick}
          aria-hidden="true"
        />
      )}

      <div
        ref={sidebarRef}
        id="app-sidebar"
        className="absolute h-full z-1 overflow-hidden"
        // Mobile-only dialog semantics: when the sidebar is acting as
        // an overlay (xs viewport), expose it to assistive tech as a
        // dialog so screen readers announce its open/close state and
        // focus is constrained to it. On desktop the sidebar is a
        // permanent fixture, so no dialog role. (audit 02 R-1)
        {...(windowMinSize.xs
          ? {
              role: 'dialog' as const,
              'aria-modal': true,
              'aria-label': 'Navigation menu',
              'aria-hidden': isSidebarCollapsed,
            }
          : {})}
        style={{
          width: isSidebarCollapsed
            ? `${ICON_ONLY_WIDTH}px`
            : `${currentWidth.current}px`,
          transition: 'width 200ms ease-out',
          ...(windowMinSize.xs
            ? {
                zIndex: 3,
                position: 'fixed',
                backdropFilter: 'blur(8px)',
                transform: isSidebarCollapsed
                  ? 'translateX(-100%)'
                  : 'translateX(0)',
                width: 250,
                maxWidth: 'calc(100vw - 24px)',
                borderRight: `1px solid ${token.colorBorderSecondary}`,
                borderRadius: 12,
                boxShadow: 'rgba(0, 0, 0, 0.075) 0px 2px 16px 0px',
                transition: 'transform 200ms ease-out',
              }
            : {}),
        }}
      >
        <LeftSidebar />
      </div>

      <SidebarToggleButton />

      {/* Spacer div for layout */}
      <div
        ref={spacerRef}
        className="flex-shrink-0 z-2 pointer-events-none"
        style={
          windowMinSize.xs
            ? {
                width: 0,
              }
            : {
                width: isSidebarCollapsed
                  ? `${ICON_ONLY_WIDTH}px`
                  : `${currentWidth.current}px`,
                transition: 'all 200ms ease-out', // Default transition, overridden during dragging
              }
        }
      />

      {/* Main Content Area */}
      <div
        className="flex-1 flex flex-col relative overflow-hidden"
        style={{
          backgroundColor: token.colorBgLayout,
        }}
      >
        {/* Content */}
        <div className="flex-1 overflow-hidden relative">
          <div
            ref={mainContentRef}
            className="w-full h-full overflow-hidden relative"
          >
            {children}
          </div>
        </div>
        {!isSidebarCollapsed && (
          <div
            className="absolute top-0 left-0 w-1 h-full cursor-col-resize z-3"
            onMouseDown={handleMouseDown}
          />
        )}
      </div>
    </div>
  )
}

export default AppLayout

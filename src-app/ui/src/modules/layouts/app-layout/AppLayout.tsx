import React, { useCallback, useEffect, useLayoutEffect, useRef } from 'react'
import { LeftSidebar } from '@/modules/layouts/app-layout/components/LeftSidebar'
import { SidebarToggleButton } from '@/modules/layouts/app-layout/components/SidebarToggleButton'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { cn } from '@/lib/utils'
import 'overlayscrollbars/overlayscrollbars.css'
import { Stores } from '@/core/stores'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'

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
  const { slots } = Stores.ModuleSystem
  const appBanners = [...(slots.get('appBanners') || [])].sort(
    (a, b) => (a.order ?? 0) - (b.order ?? 0),
  )
  const windowMinSize = useWindowMinSize()

  const sidebarRef = useRef<HTMLDivElement>(null)
  const spacerRef = useRef<HTMLDivElement>(null)
  const mainContentRef = useRef<HTMLDivElement>(null)
  // Seed the local ref from the persistent store. Each route's
  // `*Layout` component mounts its OWN `<AppLayout>` instance, so a
  // plain `useRef(200)` reset the sidebar width every time the user
  // navigated. The ref stays for fast drag-time writes (no React
  // re-renders), and we sync back to the store on drag end so the
  // next mount picks up the same width.
  const currentWidth = useRef(Stores.AppLayout.__state.sidebarWidth)

  const MIN_WIDTH = 200
  const MAX_WIDTH = 400
  // Collapsed sidebar fully disappears on desktop (mobile already does
  // via translateX(-100%) below). The toggle button lives outside the
  // sidebar (SidebarToggleButton, fixed-positioned) so reopening still
  // works at any width.
  const COLLAPSED_WIDTH = 0

  // Single source of truth for the sidebar's transition string.
  // Both the React-managed style prop below AND every imperative
  // `sidebarRef.current.style.transition = ...` write must use this
  // exact value when re-enabling transitions — otherwise the
  // transition list drops properties (notably `transform`, which
  // drives the xs slide-in) and the next state change snaps
  // instantly instead of animating.
  const SIDEBAR_TRANSITION =
    'width 200ms ease-out, transform 200ms ease-out, box-shadow 200ms ease-out'
  const SPACER_TRANSITION = 'all 200ms ease-out'

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
            spacerRef.current.style.transition = SPACER_TRANSITION
          }
          // Clear the imperative width override so React's next
          // re-render (with `width: currentWidth.current`) starts
          // the CSS transition from the user's settled width
          // instead of whatever narrow drag value we just wrote.
          // Without this, the sidebar would visibly grow back to
          // full width while sliding left.
          if (sidebarRef.current) {
            sidebarRef.current.style.width = ''
          }
          Stores.AppLayout.setSidebarCollapsed(true)
        } else if (newWidth >= MIN_WIDTH && newWidth <= MAX_WIDTH) {
          // If coming from collapsed state, re-enable transition for smooth expand
          if (isSidebarCollapsed) {
            if (spacerRef.current) {
              spacerRef.current.style.transition = SPACER_TRANSITION
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
          spacerRef.current.style.transition = SPACER_TRANSITION
        }
        if (sidebarRef.current) {
          sidebarRef.current.style.transition = SIDEBAR_TRANSITION
        }

        // Persist the final width so it survives route navigation.
        Stores.AppLayout.setSidebarWidth(currentWidth.current)

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

  // When the xs threshold flips, the sidebar's `width` and
  // `transform` both change in the same commit. With our unified
  // SIDEBAR_TRANSITION, those changes animate over 200ms and the
  // sidebar briefly appears mid-slide (width-shrink + slide-off
  // happening simultaneously). Suppress the transition for that
  // single commit so the mode-flip snaps; transitions resume the
  // next frame for normal collapse/expand animations.
  const prevXsRef = useRef(windowMinSize.xs)
  useLayoutEffect(() => {
    if (prevXsRef.current === windowMinSize.xs) return
    prevXsRef.current = windowMinSize.xs
    const sidebarEl = sidebarRef.current
    const spacerEl = spacerRef.current
    if (sidebarEl) sidebarEl.style.transition = 'none'
    if (spacerEl) spacerEl.style.transition = 'none'
    // Force a reflow so the no-transition style is committed
    // before the next style change.
    if (sidebarEl) void sidebarEl.offsetHeight
    requestAnimationFrame(() => {
      if (sidebarRef.current) {
        sidebarRef.current.style.transition = SIDEBAR_TRANSITION
      }
      if (spacerRef.current) {
        spacerRef.current.style.transition = SPACER_TRANSITION
      }
    })
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
    root.style.backgroundColor = 'var(--card)'
  }, [])

  // Visual viewport listener for mobile keyboard adjustments
  //
  // The previous version wrote `document.body.style.height` AND forced
  // `document.documentElement.scrollTop = 0` on EVERY resize event.
  // iOS Safari fires `resize` continuously while the keyboard is
  // animating in/out, so:
  //   * the unconditional scrollTop reset yanked the user to the top
  //     mid-conversation every time they tapped an input;
  //   * the body height write competed with the global app-root
  //     `height: 100dvh` rule in index.css, causing layout thrash.
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

    // Remember what was focused before the overlay opened so we can restore
    // it on close (standard dialog focus management).
    const previouslyFocused = document.activeElement as HTMLElement | null
    const sidebar = document.getElementById('app-sidebar')

    const focusable = (): HTMLElement[] => {
      if (!sidebar) return []
      return Array.from(
        sidebar.querySelectorAll<HTMLElement>(
          'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
        ),
      ).filter(el => el.offsetParent !== null)
    }

    // Move focus into the dialog on open.
    focusable()[0]?.focus()

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        Stores.AppLayout.setSidebarCollapsed(true)
        return
      }
      if (e.key !== 'Tab') return
      // Trap Tab focus inside the sidebar dialog.
      const items = focusable()
      if (items.length === 0) return
      const first = items[0]
      const last = items[items.length - 1]
      const active = document.activeElement as HTMLElement | null
      if (e.shiftKey) {
        if (active === first || !sidebar?.contains(active)) {
          e.preventDefault()
          last.focus()
        }
      } else if (active === last || !sidebar?.contains(active)) {
        e.preventDefault()
        first.focus()
      }
    }
    document.addEventListener('keydown', onKeyDown)

    return () => {
      document.body.style.overflow = previousBodyOverflow
      document.removeEventListener('keydown', onKeyDown)
      // Restore focus to the trigger that opened the overlay.
      previouslyFocused?.focus?.()
    }
  }, [windowMinSize.xs, isSidebarCollapsed])

  const handleMaskClick = () => {
    // No need to imperatively set transition here — React's
    // style prop already declares SIDEBAR_TRANSITION on every
    // render. The previous version also set transition='none'
    // 200ms later, which left the inline style at 'none'. The
    // next user-driven open would change `transform` but React's
    // transition prop didn't change (still SIDEBAR_TRANSITION),
    // so React skip-committed and the imperative 'none' stuck —
    // and the slide-in animation was lost.
    Stores.AppLayout.setSidebarCollapsed(true)
  }

  return (
    <div className="h-full w-screen flex overflow-hidden bg-card">
      {/* Keyboard skip link — first focusable element; jumps past the
          sidebar nav straight to the main content landmark. */}
      <a
        href="#main-content"
        className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:top-2 focus:left-2 focus:px-3 focus:py-1 focus:rounded focus:bg-card focus:shadow"
      >
        Skip to content
      </a>
      {/* Mask for Left Sidebar (mobile-overlay mode).
        *
        * ALWAYS mounted (no `{xs && ...}` gate) — otherwise crossing
        * the xs threshold during a window resize mounts the div
        * fresh, which fires its `transition-all` from "no value" to
        * the current opacity/background and causes a one-frame
        * flash. The desired behavior (mask only intercepts clicks
        * when the overlay is open) is just `pointer-events: auto`
        * when (xs && !collapsed); opacity stays at 0 otherwise so
        * there's nothing visible to flicker.
        *
        * Single onClick is enough; the prior triple-fire (onClick +
        * onMouseDown + onTouchStart) caused the close handler to
        * fire 2-3 times for any tap, which interacted badly with
        * the closing animation. (audit 02 R-1) */}
      <div
        // Semantic hook for build overrides (the macOS desktop
        // override applies `backdrop-filter` so the mask reads as a
        // frosted glass surface instead of a flat dim).
        data-sidebar-mask=""
        data-testid="layout-sidebar-mask"
        // Present only when the overlay is ACTUALLY active so build
        // overrides can gate filter / blur on it. Without this,
        // anything applied to `[data-sidebar-mask]` unconditionally
        // would keep filtering the whole screen even when the mask
        // is opacity:0 (the element stays mounted to avoid a
        // first-render transition flash).
        {...(windowMinSize.xs && !isSidebarCollapsed
          ? { 'data-sidebar-mask-active': '' }
          : {})}
        // Standard shadcn overlay (matches Dialog/Sheet): a faint tint + blur —
        // not a custom card-tinted mask. Always mounted; visibility toggles via
        // opacity (no first-render transition flash), and opacity:0 also hides
        // the backdrop blur so it never filters the screen while closed.
        className={cn(
          'fixed h-full w-full z-3 bg-black/10 supports-backdrop-filter:backdrop-blur-xs transition-opacity duration-200',
          windowMinSize.xs && !isSidebarCollapsed
            ? 'opacity-100 pointer-events-auto'
            : 'opacity-0 pointer-events-none',
        )}
        onClick={handleMaskClick}
        aria-hidden="true"
      />

      <div
        ref={sidebarRef}
        id="app-sidebar"
        data-testid="app-sidebar"
        // Mobile-only dialog semantics for screen readers. Inert
        // for sighted users on desktop. (audit 02 R-1)
        role={windowMinSize.xs ? ('dialog' as const) : undefined}
        aria-modal={windowMinSize.xs ? true : undefined}
        aria-label={windowMinSize.xs ? 'Navigation menu' : undefined}
        aria-hidden={
          windowMinSize.xs ? isSidebarCollapsed : undefined
        }
        // Solid opaque surface behind the (translucent bg-muted/40) sidebar on
        // the mobile overlay — so it reads as a solid panel, not a see-through/
        // frosted one.
        className={cn(windowMinSize.xs && 'bg-background')}
        // Neutral, state-gated drop shadow (rgba black, not a brand hue) that is part of the
        // combined inline transition below; value is computed per collapse/viewport state.
        data-allow-custom-color
        // STABLE style shape: same property set in every state, only
        // the VALUES change. The previous version spread an entire
        // alternate style object when `xs` flipped — which swapped
        // the `transition` property name (`width` ↔ `transform`),
        // added/removed `position: fixed`, etc. Each of those is a
        // new style-object identity React commits to the DOM, and
        // CSS transitions fire on the new diff. Keeping the shape
        // stable means crossing the xs threshold only changes
        // `transform`, `width`, and `box-shadow` — all interpolable
        // with the single combined transition below.
        style={{
          position: windowMinSize.xs ? 'fixed' : 'absolute',
          top: 0,
          left: 0,
          height: '100%',
          overflow: 'hidden',
          zIndex: windowMinSize.xs ? 3 : 1,
          // Width stays at the user's resized value REGARDLESS of
          // collapse state on desktop — collapse is animated via
          // `transform: translateX(-100%)` so the sidebar slides off
          // to the left, not shrinks. The spacer below still
          // animates from `currentWidth → 0` to free the main
          // content area's flex space.
          width: windowMinSize.xs ? 250 : currentWidth.current,
          maxWidth: windowMinSize.xs ? 'calc(100vw - 24px)' : undefined,
          transform: isSidebarCollapsed
            ? 'translateX(-100%)'
            : 'translateX(0)',
          borderRight: windowMinSize.xs
            ? `1px solid var(--border)`
            : undefined,
          // Box-shadow extends ~16px past the wrapper edges. When the
          // wrapper translates offscreen on collapse (translateX(-100%)
          // on xs), the right-edge tail of that 16px-blur shadow re-
          // enters the visible viewport as a phantom 16px stripe along
          // the screen's left side. Gating on `!isSidebarCollapsed`
          // means there's no shadow when there's nothing to bleed
          // from. The shadow fades back in on slide-out via
          // SIDEBAR_TRANSITION's `box-shadow 200ms ease-out`.
          boxShadow:
            windowMinSize.xs && !isSidebarCollapsed
              ? 'rgba(0, 0, 0, 0.075) 0px 2px 16px 0px'
              : 'none',
          // Single transition spanning every value that can change
          // on the xs threshold flip. Property name stays constant,
          // so the browser doesn't reset mid-flight. Kept in sync
          // with the imperative writes above via SIDEBAR_TRANSITION.
          transition: SIDEBAR_TRANSITION,
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
                  ? `${COLLAPSED_WIDTH}px`
                  : `${currentWidth.current}px`,
                transition: 'all 200ms ease-out', // Default transition, overridden during dragging
              }
        }
      />

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col relative overflow-hidden bg-card">
        {/* App-wide banners (e.g. the admin "update available" notice).
            Contributed via the `appBanners` slot, so bundles that don't load a
            contributor (e.g. desktop drops server-update) render nothing. */}
        {appBanners.map((b) => (
          <LazyComponentRenderer key={b.id} component={b.component} />
        ))}
        {/* Content */}
        <div className="flex-1 overflow-hidden relative">
          <section
            ref={mainContentRef}
            id="main-content"
            tabIndex={-1}
            className="w-full h-full overflow-hidden relative"
          >
            {children}
          </section>
        </div>
        {!isSidebarCollapsed && (
          <div
            // Generic semantic hook for build overrides (e.g. the
            // desktop-only floating-sidebar override) that need to
            // forward synthetic mousedowns into this handler. No
            // styling depends on it; the attribute is platform-blind.
            data-sidebar-resize-handle=""
            data-testid="layout-sidebar-resize-handle"
            className="absolute top-0 left-0 w-1 h-full cursor-col-resize z-3"
            onMouseDown={handleMouseDown}
          />
        )}
      </main>
    </div>
  )
}

export default AppLayout

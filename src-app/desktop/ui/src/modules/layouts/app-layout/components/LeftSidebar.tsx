/**
 * Desktop override of the core `LeftSidebar`.
 *
 * Resolved by Vite's `localOverridePlugin`: when desktop UI code
 * imports `@/modules/layouts/app-layout/components/LeftSidebar`, the
 * plugin sees this file at the matching path under `desktop/ui/src/`
 * and serves it INSTEAD of the core file. The core file is reached
 * via the `@ziee/ui-core/*` alias, which the override plugin does
 * NOT intercept — that's the seam.
 *
 * What this override does:
 *   - On macOS (per `isMacOS` from desktop's `platform.ts`): wraps
 *     the core sidebar in an absolutely-positioned glass panel —
 *     translucent fill via `backdrop-filter`, all-sides hairline
 *     border, 8px inset from every window edge, 10px rounded
 *     corners, subtle drop shadow. Matches the floating-sidebar
 *     look macOS apps have used since Big Sur.
 *   - On Windows / Linux: returns the core sidebar verbatim. No
 *     visual change, no extra DOM.
 *
 * What this override does NOT do:
 *   - It does not enable real desktop-show-through vibrancy. The
 *     Tauri window is opaque (`transparent: false`), so the
 *     backdrop-filter blurs whatever's inside the window behind the
 *     sidebar (the content pane). Real NSVisualEffectView vibrancy
 *     would need `window-vibrancy` on the Rust side and a
 *     transparent window — a separate, larger change.
 */

import { theme } from 'antd'
import tinycolor from 'tinycolor2'
import { useEffect } from 'react'
import { LeftSidebar as CoreLeftSidebar } from '@ziee/ui-core/modules/layouts/app-layout/components/LeftSidebar'
import { useWindowMinSize } from '@ziee/ui-core/modules/layouts/app-layout/hooks/useWindowMinSize'
import { Stores } from '@/core/stores'
import { isMacOS, isTauriView } from '@ziee/desktop/core/platform'

/**
 * Inject a `<style>` element that strips core's own chrome from the
 * `#app-sidebar` wrapper and frosts the `[data-sidebar-mask]` overlay
 * — both via `!important` so they win against React's per-render
 * inline-style writes. (An earlier version mutated `element.style.*`
 * in `useEffect`, but core re-renders the wrapper on every viewport
 * change and re-asserts its inline `borderRight` / `borderRadius` /
 * `boxShadow`, undoing our writes — visible as the right border
 * reappearing after dragging the window across the xs threshold.)
 *
 * Specifically:
 *   - Wrapper `overflow: visible` so the floating-box drop shadow
 *     isn't clipped at the wrapper edge.
 *   - Wrapper `border-right / border-radius / box-shadow: none` so
 *     none of core's outer chrome stacks on top of the floating
 *     box's own chrome (would otherwise show a visible double
 *     border or double drop shadow on every viewport).
 *   - Mask `backdrop-filter: blur(30px) saturate(180%)` so the
 *     mobile-overlay dim reads as frosted glass instead of a flat
 *     semi-opaque scrim — matches the floating sidebar's own
 *     material. Gated on the `data-sidebar-mask-active` attribute
 *     (only present while the overlay is open) — without that
 *     gate, the mask's permanently-mounted DOM node would frost the
 *     entire screen at all times.
 *
 * Gated on `enabled` so non-Mac / non-Tauri builds get nothing.
 */
function useGlassWrapperOverrides(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return
    const style = document.createElement('style')
    style.setAttribute('data-source', 'desktop-mac-glass-sidebar')
    style.textContent = `
#app-sidebar {
  overflow: visible !important;
  border-right: none !important;
  border-radius: 0 !important;
  box-shadow: none !important;
  /* Core sets backdrop-filter: blur(8px) on the wrapper in xs mode.
     The 8px gap between the viewport's left edge and our box's left
     edge would then carry an 8px blur while the box itself carries
     30px — the intensity step reads as a soft halo / shadow at the
     box edge. Kill the wrapper's blur so the gap reveals the bare
     content underneath; the box owns the only blur. */
  backdrop-filter: none !important;
  -webkit-backdrop-filter: none !important;
}
[data-sidebar-mask][data-sidebar-mask-active] {
  backdrop-filter: blur(10px) saturate(180%) !important;
  -webkit-backdrop-filter: blur(10px) saturate(180%) !important;
}
`
    document.head.appendChild(style)
    return () => {
      style.remove()
    }
  }, [enabled])
}

/**
 * AppLayout always renders its own resize handle at the content
 * area's left edge (4px wide, `cursor: col-resize`). When the glass
 * sidebar's own resize handle sits 8px to the left of that, the user
 * sees TWO col-resize cursors in the right-of-sidebar zone.
 *
 * Suppress the AppLayout one visually + interactively (cursor +
 * pointer-events) when glass is active. The forwarding handle inside
 * the box dispatches a synthetic mousedown on it programmatically,
 * which still triggers React's listener regardless of
 * `pointer-events`.
 */
function useHideAppLayoutResizeHandle(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return
    const handle = document.querySelector<HTMLElement>(
      '[data-sidebar-resize-handle]',
    )
    if (!handle) return
    const prevCursor = handle.style.cursor
    const prevPointerEvents = handle.style.pointerEvents
    handle.style.cursor = 'default'
    handle.style.pointerEvents = 'none'
    return () => {
      handle.style.cursor = prevCursor
      handle.style.pointerEvents = prevPointerEvents
    }
  }, [enabled])
}

export function LeftSidebar() {
  const { token } = theme.useToken()
  const windowMinSize = useWindowMinSize()
  const { isSidebarCollapsed } = Stores.AppLayout

  // Glass treatment fires on macOS Tauri at EVERY viewport — even
  // when the user resizes the window into the `xs` mobile-overlay
  // range. The floating-card look is the native macOS convention
  // regardless of window size; mobile-overlay was a web concern.
  const glassActive = isTauriView && isMacOS
  // On `xs`, the wrapper switches to mobile-overlay mode (position
  // fixed, slide in/out over content). Resize-by-drag is meaningless
  // in that mode — the wrapper width is locked at 250 by core — so
  // hide our forwarding handle there.
  const showResizeHandle = !windowMinSize.xs
  useGlassWrapperOverrides(glassActive)
  useHideAppLayoutResizeHandle(glassActive)

  // Glass treatment is gated on BOTH `isTauriView` and `isMacOS`:
  //   - `isTauriView` is required because the desktop UI is also
  //     served over the Remote Access ngrok tunnel to remote browsers
  //     (`tunnel_auth` module). A Mac user opening the tunnel URL
  //     from Safari is on a browser, not the Tauri webview, and the
  //     glass styling would be wrong there (no window chrome, no
  //     native sidebar convention to imitate).
  //   - `isMacOS` is required because the floating-card sidebar is
  //     specifically a macOS Sonoma+ convention; Windows / Linux
  //     desktops keep the flush sidebar.
  // Same goes for the mobile-overlay sidebar mode (`xs`) — the core
  // overlay treatment is right for that case.
  if (!glassActive) {
    return <CoreLeftSidebar />
  }

  // Hand the core sidebar a transparent inner so the glass shows
  // through. The wrapper handles positioning, rounded corners,
  // border, and the backdrop-filter.
  //
  // Layout: 8px inset on ALL FOUR sides so the box reads as a
  // free-floating card with a complete drop shadow on every edge.
  // The existing AppLayout resize handle sits 8px to the right of
  // the box (at the content area's left edge); user grabs in that
  // 8px gap to resize.
  return (
    <div
      style={{
        position: 'absolute',
        top: 8,
        left: 8,
        right: 8,
        bottom: 8,
        borderRadius: 10,
        overflow: 'hidden',
        // Alpha'd `colorBgContainer` (white) so the glass tint stays
        // brighter than the surrounding off-white surfaces. In dark
        // mode this picks up the dark container hue automatically.
        backgroundColor: tinycolor(token.colorBgContainer)
          .setAlpha(0.6)
          .toRgbString(),
        backdropFilter: 'blur(30px) saturate(180%)',
        WebkitBackdropFilter: 'blur(30px) saturate(180%)',
        // All-sides 1px border via inset shadow (stays inside the
        // rounded corners), top highlight that sells the glass
        // material, plus a visible outer drop shadow that lifts the
        // card off the white background on every edge. Border uses
        // the theme's lighter `colorBorderSecondary` token so it
        // reads as a soft hairline (the full-strength `colorBorder`
        // came through too dark against the alpha'd glass fill).
        //
        // Drop shadow drops when:
        //   1) the sidebar is collapsed — otherwise the shadow that
        //      hangs off the box's right edge (now slid offscreen
        //      via translateX(-100%)) would peek into the visible
        //      viewport as a phantom shadow along the content's
        //      left edge;
        //   2) on `xs` (mobile-overlay mode) — the wrapper is
        //      anchored to the viewport's left edge, so the shadow's
        //      left tail bleeds against the screen edge and reads
        //      as a stripe of shadow against the content. The inset
        //      border + highlight alone are enough to sell the box's
        //      edge in that context.
        // Inset border + highlight stay so the sidebar's edge still
        // reads when sliding back in. Transition box-shadow to match
        // the slide tempo.
        boxShadow:
          isSidebarCollapsed || windowMinSize.xs
            ? `inset 0 0 0 1px ${token.colorBorderSecondary}, ` +
              'inset 0 1px 0 rgba(255, 255, 255, 0.30)'
            : `inset 0 0 0 1px ${token.colorBorderSecondary}, ` +
              'inset 0 1px 0 rgba(255, 255, 255, 0.30), ' +
              // Softened drop shadow — was rgba(0,0,0,0.12). The
              // original carried more weight than the macOS floating
              // sidebar convention; halved + slightly contained
              // makes the lift feel like a hint instead of a lifted
              // card on a paper background.
              '0 2px 8px rgba(0, 0, 0, 0.05)',
        transition: 'box-shadow 200ms ease-out',
      }}
    >
      <CoreLeftSidebar
        rootStyle={{
          // Strip the core defaults so the glass wrapper carries
          // the surface treatment.
          backgroundColor: 'transparent',
          borderRight: 'none',
        }}
      />

      {/* Resize handle pinned to the box's right edge. The actual
          width-mutation logic lives in AppLayout's own handle (4px
          wide, sits at the content area's left edge — 8px past our
          box). We forward our mousedown into a native MouseEvent
          dispatched on that handle, so AppLayout's React listener
          fires and runs unchanged. Keeps the resize math in one
          place (core) instead of duplicating it on the desktop
          side.

          Why not `parentLevel` via the shared `ResizeHandle`? It
          mutates the parent's `style.width` directly, which races
          against AppLayout's `currentWidth.current` ref-tracking
          (the ref re-asserts on next render). Forwarding the event
          re-uses AppLayout's full lifecycle including the ref
          update.

          Hidden on `xs` — mobile-overlay sidebar is non-resizable. */}
      {showResizeHandle && (<div
        className="absolute top-0 right-0 w-1 h-full cursor-col-resize z-10"
        onMouseDown={event => {
          const realHandle = document.querySelector<HTMLElement>(
            '[data-sidebar-resize-handle]',
          )
          if (!realHandle) return
          // Bubbling + cancelable so React's delegated listener at
          // the document picks it up exactly like a real mousedown.
          realHandle.dispatchEvent(
            new MouseEvent('mousedown', {
              bubbles: true,
              cancelable: true,
              clientX: event.clientX,
              clientY: event.clientY,
              button: event.button,
              buttons: event.buttons,
            }),
          )
          // We forwarded the event; suppress the original so it
          // doesn't also bubble into the floating box.
          event.preventDefault()
          event.stopPropagation()
        }}
      />)}
    </div>
  )
}

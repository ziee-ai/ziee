import { Stores } from '@/core/stores'

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
  const { isSidebarCollapsed } = Stores.AppLayout

  // Theme-aware soft-fade: top stop = the `--card` token, bottom stop = that SAME
  // color at alpha 0 (color-mix in oklab toward transparent), so it fades through
  // the surface hue rather than through transparent black (no gray midpoint), and
  // follows the active theme / dark mode instead of a hardcoded white.
  const fadeGradient = `linear-gradient(to bottom, var(--card), color-mix(in oklab, var(--card) 0%, transparent))`

  return (
    <div
      className={`h-[50px] w-full flex relative px-3 transition-all duration-200 ease-in-out box-border py-0 ${className}`}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
        // Lift above the page-content sibling so the gradient
        // overlay (absolutely positioned at top: 100%, extending
        // INTO the content area's space) actually paints over it.
        // Without this z-index, the later-in-DOM content area
        // takes precedence and the gradient disappears under it.
        zIndex: 2,
        ...style,
      }}
    >
      {children}
      {/* Soft-fade overlay just below the header. Top edge is the
          content surface color (opaque), bottom edge is transparent.
          When content scrolls up beneath the (transparent) header,
          it appears to dissolve into the bg color before being
          clipped by the header — smoother than a hard line. */}
      <div
        aria-hidden="true"
        data-testid="layout-header-fade-overlay"
        // Theme-token gradient (card surface → same color at alpha 0); a two-stop
        // gradient can't be expressed as a single semantic utility class.
        data-allow-custom-color
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: '100%',
          height: 16,
          pointerEvents: 'none',
          background: fadeGradient,
        }}
      />
    </div>
  )
}

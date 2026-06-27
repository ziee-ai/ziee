import tinycolor from 'tinycolor2'
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
  const bgCard = '#ffffff'
  const { isSidebarCollapsed } = Stores.AppLayout

  // Same-color alpha-faded transparent — pairs better with the
  // bg-color top stop than the CSS `transparent` keyword, which
  // interpolates through transparent BLACK and can produce a faint
  // gray midpoint on light themes.
  const fadeOut = tinycolor(bgCard).setAlpha(0).toRgbString()

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
        style={{
          position: 'absolute',
          left: 0,
          right: 0,
          top: '100%',
          height: 16,
          pointerEvents: 'none',
          background: `linear-gradient(to bottom, ${bgCard}, ${fadeOut})`,
        }}
      />
    </div>
  )
}

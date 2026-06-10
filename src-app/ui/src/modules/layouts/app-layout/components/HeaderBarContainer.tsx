import { theme } from 'antd'
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
  const { token } = theme.useToken()
  const { isSidebarCollapsed } = Stores.AppLayout

  // Same-color alpha-faded transparent — pairs better with the
  // bg-color top stop than the CSS `transparent` keyword, which
  // interpolates through transparent BLACK and can produce a faint
  // gray midpoint on light themes.
  const fadeOut = tinycolor(token.colorBgContainer).setAlpha(0).toRgbString()

  return (
    <div
      className={`h-[50px] w-full flex relative px-3 transition-all duration-200 ease-in-out box-border py-0 ${className}`}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
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
          background: `linear-gradient(to bottom, ${token.colorBgContainer}, ${fadeOut})`,
        }}
      />
    </div>
  )
}

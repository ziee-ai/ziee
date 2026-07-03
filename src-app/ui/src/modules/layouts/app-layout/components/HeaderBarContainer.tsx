import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'

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
  const { isSidebarCollapsed, nativeScroll } = Stores.AppLayout

  // Native document-scroll (mobile Settings): the header is NORMAL-FLOW content
  // (position: relative) so it scrolls fully up and away with the page — which
  // is what lets the content below flow under the notch / URL bar (a sticky or
  // fixed header pins to the top and blocks that). It fills the notch strip at
  // rest via a safe-area-top pad, and reappears when you scroll back to the top.
  // Default (fixed-shell) mode is a plain static header — byte-identical.
  return (
    <div
      className={cn(
        'w-full flex px-3 border-b border-border box-border py-0',
        nativeScroll
          ? 'relative bg-card'
          : 'h-[50px] relative transition-all duration-200 ease-in-out',
        className,
      )}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
        zIndex: 2,
        ...(nativeScroll
          ? {
              // fill the notch strip; keep the 50px control row below it
              paddingTop: 'env(safe-area-inset-top, 0px)',
              height: 'calc(env(safe-area-inset-top, 0px) + 50px)',
            }
          : null),
        ...style,
      }}
    >
      {children}
    </div>
  )
}

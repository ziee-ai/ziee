import { useEffect, useRef, useState } from 'react'
import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'

interface HeaderBarContainerProps {
  children?: React.ReactNode
  className?: string
  style?: React.CSSProperties
}

/** Header row height below the safe-area strip (matches the default h-[50px]). */
export const NATIVE_HEADER_ROW = 50
/** Full header height incl. the iOS safe-area top inset — pages offset content by this. */
export const NATIVE_HEADER_OFFSET = `calc(env(safe-area-inset-top, 0px) + ${NATIVE_HEADER_ROW}px)`

export const HeaderBarContainer = ({
  children,
  className = '',
  style = {},
}: HeaderBarContainerProps) => {
  const { isSidebarCollapsed, nativeScroll } = Stores.AppLayout

  // Native document-scroll (mobile Settings), modeled on asurascans' reader:
  // a FIXED, translucent (bg-card/95 + backdrop-blur) header that AUTOHIDES —
  // slides up on scroll-down, returns on scroll-up. viewport-fit=cover (set in
  // index.html) is what lets content flow under the notch even with a fixed
  // header. Pages offset content by NATIVE_HEADER_OFFSET so the first item isn't
  // hidden behind it at rest. Default mode = plain static header (byte-identical).
  const [hidden, setHidden] = useState(false)
  const lastY = useRef(0)
  useEffect(() => {
    if (!nativeScroll) {
      setHidden(false)
      return
    }
    lastY.current = window.scrollY
    const onScroll = () => {
      const y = window.scrollY
      if (y > lastY.current && y > NATIVE_HEADER_ROW) setHidden(true)
      else if (y < lastY.current) setHidden(false)
      lastY.current = y
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [nativeScroll])

  return (
    <div
      className={cn(
        'w-full flex px-3 border-b border-border box-border py-0',
        nativeScroll
          ? 'fixed top-0 inset-x-0 bg-card/95 backdrop-blur-md transition-transform duration-300 ease-in-out'
          : 'h-[50px] relative transition-all duration-200 ease-in-out',
        className,
      )}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
        zIndex: nativeScroll ? 30 : 2,
        ...(nativeScroll
          ? {
              paddingTop: 'env(safe-area-inset-top, 0px)',
              height: NATIVE_HEADER_OFFSET,
              transform: hidden ? 'translateY(-100%)' : 'translateY(0)',
            }
          : null),
        ...style,
      }}
    >
      {children}
    </div>
  )
}

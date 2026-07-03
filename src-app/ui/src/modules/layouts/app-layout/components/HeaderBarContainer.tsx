import { useEffect, useRef, useState } from 'react'
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

  // Native document-scroll (mobile Settings): the header is sticky and autohides
  // — slides up when scrolling DOWN (immersive), returns on scroll UP. It also
  // pads its top by the iOS safe-area inset so it fills the notch strip. In the
  // default (fixed-shell) mode it's a plain static header, no listener.
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
      if (y > lastY.current && y > 60) setHidden(true)
      else if (y < lastY.current) setHidden(false)
      lastY.current = y
    }
    window.addEventListener('scroll', onScroll, { passive: true })
    return () => window.removeEventListener('scroll', onScroll)
  }, [nativeScroll])

  return (
    <div
      className={cn(
        'h-[50px] w-full flex px-3 border-b border-border box-border py-0',
        nativeScroll
          ? 'sticky top-0 bg-card transition-transform duration-300 ease-in-out'
          : 'relative transition-all duration-200 ease-in-out',
        className,
      )}
      style={{
        paddingLeft: isSidebarCollapsed ? 48 : 12,
        paddingRight: 12,
        zIndex: nativeScroll ? 30 : 2,
        ...(nativeScroll
          ? {
              // fill the notch strip; keep the 50px control row below it
              paddingTop: 'env(safe-area-inset-top, 0px)',
              height: 'calc(env(safe-area-inset-top, 0px) + 50px)',
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

import { useEffect, useRef, useState } from 'react'
import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'

interface HeaderBarContainerProps {
  children?: React.ReactNode
  className?: string
  style?: React.CSSProperties
}

/** Scroll distance (px) before the header starts hiding on scroll-down. */
const HIDE_THRESHOLD = 50

export const HeaderBarContainer = ({
  children,
  className = '',
  style = {},
}: HeaderBarContainerProps) => {
  const { isSidebarCollapsed, nativeScroll } = Stores.AppLayout

  // Native document-scroll (mobile Settings): EXPERIMENT — direction-based with
  // FIXED (vs sticky):
  //  • default / scrolling DOWN → position:relative → header wipes away, notch
  //    region freed so content flows under it.
  //  • scrolling UP → position:fixed top:0 → pins back into view.
  // fixed is fully out of flow (unlike sticky, which reserves a box); testing
  // whether that avoids Safari's "top occupied" under-notch latch.
  const [pinned, setPinned] = useState(false)
  const lastY = useRef(0)
  useEffect(() => {
    if (!nativeScroll) {
      setPinned(false)
      return
    }
    lastY.current = window.scrollY
    const onScroll = () => {
      const y = window.scrollY
      if (y > lastY.current && y > HIDE_THRESHOLD) setPinned(false) // down → relative (wipes away)
      else if (y < lastY.current) setPinned(true) // up → fixed (reappears)
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
          ? cn('bg-card', pinned ? 'sticky' : 'relative')
          : 'h-[50px] relative transition-all duration-200 ease-in-out',
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
              // DIAGNOSTIC: top:20 on the fixed state — if the header anchors
              // 20px from the VIEWPORT top it's free; if from lower down, a
              // containing-block ancestor is trapping it.
              ...(pinned ? { top: 1 } : null),
            }
          : null),
        ...style,
      }}
    >
      {children}
    </div>
  )
}

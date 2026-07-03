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

  // Native document-scroll (mobile Settings): DIRECTION-BASED header —
  //  • scrolling DOWN  → position:relative, so the header scrolls up and away
  //    with the page and frees the notch region (content flows under it).
  //  • scrolling UP    → position:sticky top:0, so it pins back into view.
  // sticky reserves the same flow slot as relative, so toggling doesn't reflow
  // the content. This gives BOTH content-under-notch (on the down-scroll, where
  // it matters) AND reappear-on-scroll-up — without a fixed header (which the
  // shell traps) or a portal (which wrecks tab order). Default mode = static.
  const [pinned, setPinned] = useState(true)
  const lastY = useRef(0)
  useEffect(() => {
    if (!nativeScroll) {
      setPinned(true)
      return
    }
    lastY.current = window.scrollY
    const onScroll = () => {
      const y = window.scrollY
      if (y <= HIDE_THRESHOLD) setPinned(true) // near the top → always shown
      else if (y > lastY.current) setPinned(false) // scrolling down → relative (away)
      else if (y < lastY.current) setPinned(true) // scrolling up → sticky (appears)
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
          ? cn('top-0 bg-card', pinned ? 'sticky' : 'relative')
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
            }
          : null),
        ...style,
      }}
    >
      {children}
    </div>
  )
}

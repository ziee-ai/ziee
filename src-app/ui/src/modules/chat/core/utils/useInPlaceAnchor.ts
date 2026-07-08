import { useCallback, type RefObject } from 'react'
import {
  findScrollParent,
  inPlaceAnchorDelta,
} from '@/modules/chat/core/utils/scrollAnchor.utils'

/**
 * message-scroll-stability ITEM-7 — pin a row's top edge across an INTENTIONAL
 * in-place height change (show-more expand/collapse, inline-file drag-resize).
 *
 * Returns a `beforeChange()` callback: call it in the click / pointer handler
 * BEFORE mutating the height-affecting state. It captures the row's top relative
 * to its scroll viewport, then — after React paints AND
 * `@tanstack/react-virtual`'s `measureElement` ResizeObserver has re-measured
 * (two rAFs) — trims any residual drift so the row's top holds exactly. The pure
 * `inPlaceAnchorDelta` guard makes this a NO-OP when the virtualizer already
 * owns the correction (row above the fold) or nothing visible moved (row below
 * the fold), so the two mechanisms never double-adjust (angle C).
 *
 * Self-contained (finds its own scroll parent) so it works from inside a
 * virtualized `ChatMessage` row without threading the MessageList scroll element
 * down through the file module.
 */
export function useInPlaceAnchor(rowRef: RefObject<HTMLElement | null>) {
  return useCallback(() => {
    const el = rowRef.current
    if (!el) return
    const scroller = findScrollParent(el)
    const viewportTop = scroller ? scroller.getBoundingClientRect().top : 0
    const viewportHeight = scroller ? scroller.clientHeight : window.innerHeight
    const topBefore = el.getBoundingClientRect().top - viewportTop
    requestAnimationFrame(() =>
      requestAnimationFrame(() => {
        const cur = rowRef.current
        if (!cur) return
        const topAfter = cur.getBoundingClientRect().top - viewportTop
        const delta = inPlaceAnchorDelta(topBefore, topAfter, viewportHeight)
        if (delta === 0) return
        if (scroller) scroller.scrollTop += delta
        else window.scrollBy(0, delta)
      }),
    )
  }, [rowRef])
}

import { useCallback, useEffect, useRef, type RefObject } from 'react'
import {
  findScrollParent,
  inPlaceAnchorDelta,
} from '@/modules/chat/core/utils/scrollAnchor.utils'

/** A correction larger than this almost certainly means the virtualizer is
 *  mid-adjusting its own scroll for an above-fold size change — do NOT layer a
 *  second correction on top (that is the double-adjust jump we're avoiding). We
 *  only trim small residual drift. */
const MAX_INPLACE_CORRECTION_PX = 48

/**
 * message-scroll-stability ITEM-7 — pin a row's top edge across an INTENTIONAL
 * in-place height change (show-more expand/collapse, inline-file drag-resize).
 *
 * Returns a `beforeChange()` callback: call it in the click / pointer handler
 * BEFORE mutating the height-affecting state. It captures the row's top relative
 * to its scroll viewport, then — after React paints AND
 * `@tanstack/react-virtual`'s `measureElement` ResizeObserver has re-measured
 * (two rAFs) — trims any small residual drift so the row's top holds. The pure
 * `inPlaceAnchorDelta` guard defers to the virtualizer for above/below-fold rows
 * so the two mechanisms never double-adjust (angle C); a large delta (the
 * virtualizer still mid-adjusting) is skipped for the same reason. Overlapping
 * calls (rapid re-toggles) cancel the prior pending rAF chain so they can't
 * stack corrections. Self-contained (finds its own scroll parent) so it works
 * from inside a virtualized `ChatMessage` row without threading a scroll element
 * across the chat↔file module boundary.
 */
export function useInPlaceAnchor(rowRef: RefObject<HTMLElement | null>) {
  const pending = useRef<{ raf1: number; raf2: number } | null>(null)
  const cancel = () => {
    const p = pending.current
    if (!p) return
    cancelAnimationFrame(p.raf1)
    cancelAnimationFrame(p.raf2)
    pending.current = null
  }
  useEffect(() => cancel, [])

  return useCallback(() => {
    const el = rowRef.current
    if (!el) return
    cancel() // a new intentional change supersedes any in-flight correction
    const scroller = findScrollParent(el)
    const viewportTop = scroller ? scroller.getBoundingClientRect().top : 0
    const viewportHeight = scroller ? scroller.clientHeight : window.innerHeight
    const topBefore = el.getBoundingClientRect().top - viewportTop
    const raf1 = requestAnimationFrame(() => {
      const raf2 = requestAnimationFrame(() => {
        pending.current = null
        const cur = rowRef.current
        if (!cur) return
        const topAfter = cur.getBoundingClientRect().top - viewportTop
        const delta = inPlaceAnchorDelta(topBefore, topAfter, viewportHeight)
        if (delta === 0 || Math.abs(delta) > MAX_INPLACE_CORRECTION_PX) return
        if (scroller) scroller.scrollTop += delta
        else window.scrollBy(0, delta)
      })
      pending.current = { raf1, raf2 }
    })
    pending.current = { raf1, raf2: 0 }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rowRef])
}

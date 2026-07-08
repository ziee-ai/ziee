import { useCallback, useEffect, useRef, type RefObject } from 'react'
import {
  findScrollParent,
  inPlaceAnchorDelta,
} from '@/modules/chat/core/utils/scrollAnchor.utils'

/**
 * Shared signal between `useInPlaceAnchor` and `MessageList`'s virtualizer
 * (message-scroll-stability ITEM-7). When the user intentionally changes a row's
 * height in place (show-more expand/collapse, inline-file resize), the row's
 * `virtualizer key` (its message id) is parked here for the duration of the
 * change. `MessageList` passes `shouldAdjustScrollPositionOnItemSizeChange` a
 * predicate that returns FALSE for the parked key — suppressing
 * `@tanstack/react-virtual`'s own above-fold scroll compensation for THAT row
 * only — so the row simply grows downward from its current top instead of the
 * virtualizer teleporting the viewport (the ~1300px jump when the expanding row
 * straddles the viewport-top fold). The hook then pins any residual drift.
 *
 * A single module singleton is sufficient: only one MessageList is mounted at a
 * time, and an in-place change is a synchronous user gesture bounded by two rAFs.
 */
export const inPlaceAnchorSignal: { key: string | null } = { key: null }

/**
 * message-scroll-stability ITEM-7 — pin a row's top edge across an INTENTIONAL
 * in-place height change.
 *
 * Returns a `beforeChange()` callback: call it in the click / pointer / key
 * handler BEFORE mutating the height-affecting state. It (1) parks the enclosing
 * message's virtualizer key in `inPlaceAnchorSignal` so MessageList suppresses
 * the virtualizer's auto scroll-adjust for that row, (2) captures the row's top
 * relative to its scroll viewport, then (3) after React paints AND the
 * `measureElement` ResizeObserver has re-measured (two rAFs) pins the row's top
 * back to where it was and unparks the key. Overlapping calls cancel the prior
 * pending chain. Self-contained (finds its own scroll parent + message key) so
 * it works from inside a virtualized `ChatMessage` row without threading a
 * scroll element across the chat↔file module boundary.
 */
export function useInPlaceAnchor(rowRef: RefObject<HTMLElement | null>) {
  const pending = useRef<{ raf1: number; raf2: number } | null>(null)
  const cancel = () => {
    const p = pending.current
    if (!p) return
    cancelAnimationFrame(p.raf1)
    cancelAnimationFrame(p.raf2)
    pending.current = null
    inPlaceAnchorSignal.key = null
  }
  useEffect(() => cancel, [])

  return useCallback(() => {
    const el = rowRef.current
    if (!el) return
    cancel() // a new intentional change supersedes any in-flight correction
    // The virtualizer keys rows by message id; the row wrapper / ChatMessage
    // carries it as data-message-id. Park it so MessageList suppresses the
    // virtualizer's auto-adjust for this row only.
    const msgEl = el.closest<HTMLElement>('[data-message-id]')
    inPlaceAnchorSignal.key = msgEl?.dataset.messageId ?? null
    const scroller = findScrollParent(el)
    const viewportTop = scroller ? scroller.getBoundingClientRect().top : 0
    const viewportHeight = scroller ? scroller.clientHeight : window.innerHeight
    const topBefore = el.getBoundingClientRect().top - viewportTop
    const raf1 = requestAnimationFrame(() => {
      const raf2 = requestAnimationFrame(() => {
        pending.current = null
        inPlaceAnchorSignal.key = null
        const cur = rowRef.current
        if (!cur) return
        const topAfter = cur.getBoundingClientRect().top - viewportTop
        const delta = inPlaceAnchorDelta(topBefore, topAfter, viewportHeight)
        if (delta === 0) return
        if (scroller) scroller.scrollTop += delta
        else window.scrollBy(0, delta)
      })
      pending.current = { raf1, raf2 }
    })
    pending.current = { raf1, raf2: 0 }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [rowRef])
}

/**
 * Scroll-anchoring utilities for reverse-infinite-scroll (ITEM-8 / DEC-2).
 *
 * When older messages are PREPENDED at the top, the content above the viewport
 * grows and the browser would teleport the view. To keep the previously-visible
 * content pinned we: (1) capture the top-most visible message + its offset from
 * the viewport top BEFORE prepend, (2) after prepend re-measure that same
 * message and scroll by the delta so it lands back at the same offset.
 *
 * The MATH (`pickTopAnchor`, `restoreDelta`) is pure and unit-tested (TEST-2)
 * with synthetic rects; the DOM readers below are thin wrappers over
 * `getBoundingClientRect` used by the page.
 */

export interface MessageBox {
  id: string
  /** client-Y of the element's top edge (getBoundingClientRect().top). */
  top: number
  /** client-Y of the element's bottom edge. */
  bottom: number
}

export interface ScrollAnchor {
  anchorId: string
  /** Offset of the anchor's top edge below the viewport top, in px. */
  savedTop: number
}

/**
 * Pure: choose the anchor = the first message box that is at least partially
 * visible (its bottom is below the viewport top). `savedTop` is that box's top
 * relative to the viewport top (can be negative if the box starts above the
 * fold but is still partially visible). Returns null when nothing qualifies.
 */
export function pickTopAnchor(
  boxes: MessageBox[],
  viewportTop: number,
): ScrollAnchor | null {
  for (const box of boxes) {
    if (box.bottom > viewportTop) {
      return { anchorId: box.id, savedTop: box.top - viewportTop }
    }
  }
  return null
}

/**
 * Pure: how far to scroll so an anchor whose top is now at `newTop` (relative to
 * viewport top) returns to its captured `savedTop`. Positive → scroll down.
 */
export function restoreDelta(savedTop: number, newTop: number): number {
  return newTop - savedTop
}

/**
 * Pure (virtualized list, ITEM-4): the scroll OFFSET that re-pins an anchor row
 * at its captured `viewportOffset` given the virtualizer's content-space offset
 * for that row's (post-prepend) index. Clamped ≥ 0 (can't scroll above the top).
 * `offsetForIndex` = `virtualizer.getOffsetForIndex(index, 'start')[0]` (the
 * row's top in content coordinates); `viewportOffset` = the row's top relative
 * to the viewport top when captured (may be negative if it straddled the fold).
 */
export function indexRestoreOffset(
  offsetForIndex: number,
  viewportOffset: number,
): number {
  return Math.max(0, offsetForIndex - viewportOffset)
}

/**
 * Pure (message-scroll-perf ITEM-6, DEC-6): whether an EXPLICIT anchor restore
 * is still needed after a prepend, given the scroller's current offset and the
 * target restore offset. `@tanstack/react-virtual` already adjusts scroll for
 * above-viewport rows whose size changes (the prepended rows settling from
 * estimate→measured), so once the anchor is within `tolerance` px of its target
 * an extra `scrollToOffset` is a redundant no-op that could itself nudge the
 * view — skip it. This makes the manual restore idempotent on top of the
 * virtualizer's own correction (no double-adjust jump).
 */
export function anchorRestoreNeeded(
  currentOffset: number,
  targetOffset: number,
  tolerance = 2,
): boolean {
  return Math.abs(currentOffset - targetOffset) > tolerance
}

// ── DOM readers (thin; the pure math above is what tests exercise) ───────────

/** Read every `[data-message-id]` box under `container`, in document order. */
export function readMessageBoxes(container: HTMLElement): MessageBox[] {
  const els = container.querySelectorAll<HTMLElement>('[data-message-id]')
  const boxes: MessageBox[] = []
  els.forEach(el => {
    const id = el.getAttribute('data-message-id')
    if (!id) return
    const rect = el.getBoundingClientRect()
    boxes.push({ id, top: rect.top, bottom: rect.bottom })
  })
  return boxes
}

/** client-Y of the top edge of the message with `id`, or null if not present. */
export function measureMessageTop(
  container: HTMLElement,
  id: string,
): number | null {
  const el = container.querySelector<HTMLElement>(
    `[data-message-id="${CSS.escape(id)}"]`,
  )
  if (!el) return null
  return el.getBoundingClientRect().top
}

/**
 * Capture the top-visible anchor for a scroll container. `viewportTop` is the
 * client-Y of the viewport's top edge — `0` for the window (native scroll) or
 * the scroll box's `getBoundingClientRect().top` for an inner scroller.
 */
export function captureTopAnchor(
  container: HTMLElement,
  viewportTop: number,
): ScrollAnchor | null {
  return pickTopAnchor(readMessageBoxes(container), viewportTop)
}

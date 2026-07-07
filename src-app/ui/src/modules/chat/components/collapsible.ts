/**
 * Collapse heuristics for long chat messages (ITEM-3).
 *
 * Two gates cooperate:
 *  - `shouldOfferCollapse` is a cheap PRE-gate: only long, non-streaming text is
 *    worth wrapping in a `CollapsibleBlock` at all. Streaming messages are never
 *    clamped (live tokens must stay visible — DEC-6).
 *  - `CollapsibleBlock` then MEASURES real rendered overflow and only shows the
 *    toggle when content actually exceeds `COLLAPSE_MAX_HEIGHT_PX`.
 */

/** Clamp height for a collapsed long message (~24rem). */
export const COLLAPSE_MAX_HEIGHT_PX = 384

/**
 * Character count above which a text bubble is a collapse candidate. A rough
 * proxy for "taller than the clamp"; the real decision is the runtime overflow
 * measurement in `CollapsibleBlock`. Kept generous so short/medium messages are
 * never wrapped.
 */
export const COLLAPSE_CHAR_THRESHOLD = 1200

export function shouldOfferCollapse({
  length,
  isStreaming,
}: {
  length: number
  isStreaming: boolean
}): boolean {
  if (isStreaming) return false
  return length > COLLAPSE_CHAR_THRESHOLD
}

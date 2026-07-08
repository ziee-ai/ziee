/**
 * Pure reserved-height math for inline chat file previews (message-scroll-
 * stability ITEM-2). An inline file body is given a DEFINITE height with
 * internal scroll (instead of a content-driven `max-h`), so once the header+body
 * layout exists the virtualized row's height stops changing — image decode,
 * table/xlsx parse and Shiki highlight all settle INSIDE the fixed box and are
 * zero-delta to the virtualizer. The `!inView` skeleton uses the SAME resolver,
 * so the lazy body-mount is a zero-delta swap.
 *
 * Unit-tested (TEST-2). No DOM / no React here.
 */

/** Definite body height for a generic viewer (text/markdown/image/…), px. */
export const INLINE_FILE_DEFAULT_GENERIC_PX = 400
/** Definite body height for the `inlineFill` tabular grid, px (matches the grid's
 *  own definite height it needs to row-virtualize). */
export const INLINE_FILE_DEFAULT_TABULAR_PX = 360
/** Floor for a user drag-resize. */
export const INLINE_FILE_MIN_PX = 160

/** The default reserved height for a viewer, before any user resize. */
export function defaultReservedPx(inlineFill: boolean): number {
  return inlineFill
    ? INLINE_FILE_DEFAULT_TABULAR_PX
    : INLINE_FILE_DEFAULT_GENERIC_PX
}

/** Upper bound for a user drag-resize: 80vh, but never below the generic default
 *  so a very short window still permits the default height. */
export function maxReservedPx(viewportHeight: number): number {
  return Math.max(INLINE_FILE_DEFAULT_GENERIC_PX, Math.round(viewportHeight * 0.8))
}

/** Clamp a user-chosen px into `[INLINE_FILE_MIN_PX, maxReservedPx]`. */
export function clampReservedPx(px: number, viewportHeight: number): number {
  return Math.min(Math.max(px, INLINE_FILE_MIN_PX), maxReservedPx(viewportHeight))
}

/**
 * The single definite body height used by BOTH the mounted body and the
 * `!seen` skeleton. `resizedPx` (from the persisted view state) wins when set
 * (clamped); otherwise the per-viewer default. Because both the skeleton and the
 * body call this with the same inputs, the header-only→body transition never
 * changes the row height.
 */
export function resolveBodyHeightPx(
  inlineFill: boolean,
  resizedPx: number | null,
  viewportHeight: number,
): number {
  if (resizedPx == null) return defaultReservedPx(inlineFill)
  return clampReservedPx(resizedPx, viewportHeight)
}

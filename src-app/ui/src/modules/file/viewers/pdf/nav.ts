// Pure page-navigation helpers for the PDF.js viewer toolbar (ITEM-6).
// Page tracking itself is driven by `pdfViewer.currentPageNumber` +
// the `pagechanging` EventBus event; these helpers only validate/clamp the
// user-typed jump-to-page input so the toolbar never asks the viewer for an
// out-of-range page.

/** Clamp a (possibly fractional / out-of-range) page number to `[1, numPages]`. */
export function clampPage(n: number, numPages: number): number {
  if (!Number.isFinite(n)) return 1
  const max = Math.max(numPages, 1)
  return Math.min(Math.max(Math.trunc(n), 1), max)
}

/**
 * Parse a jump-to-page input string. Returns a clamped page number, or `null`
 * when the input is not a plain positive integer (so the caller can ignore it
 * and leave the current page unchanged).
 */
export function parseJump(input: string, numPages: number): number | null {
  const t = input.trim()
  if (!/^\d+$/.test(t)) return null
  const n = Number.parseInt(t, 10)
  if (Number.isNaN(n)) return null
  return clampPage(n, numPages)
}

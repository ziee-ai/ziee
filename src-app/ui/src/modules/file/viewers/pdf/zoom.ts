// Pure zoom-step-ladder helper for the PDF.js viewer toolbar (ITEM-7).
// The discrete ladder keeps zoom-in / zoom-out predictable and is unit-testable
// in isolation from the PDFViewer instance. `page-width` / `page-fit` /
// `page-actual` are handled directly via `pdfViewer.currentScaleValue`; this
// ladder drives the +/- buttons between explicit scales.

export const ZOOM_STEPS = [
  0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0, 4.0,
] as const

export const MIN_ZOOM = ZOOM_STEPS[0]
export const MAX_ZOOM = ZOOM_STEPS[ZOOM_STEPS.length - 1]

const EPS = 1e-6

/**
 * Next scale on the ladder from `current` in direction `dir` (+1 = zoom in,
 * -1 = zoom out). A scale sitting between two steps snaps to the correct
 * neighbour; the result is clamped to `[MIN_ZOOM, MAX_ZOOM]`.
 */
export function nextZoomStep(current: number, dir: 1 | -1): number {
  if (dir === 1) {
    for (const s of ZOOM_STEPS) {
      if (s > current + EPS) return s
    }
    return MAX_ZOOM
  }
  let prev: number = MIN_ZOOM
  for (const s of ZOOM_STEPS) {
    if (s < current - EPS) prev = s
    else break
  }
  return prev
}

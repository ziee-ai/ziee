// Pure zoom/pan math for the image viewer. Extracted so the clamp/step logic is
// unit-testable without a DOM (see zoom.test.ts) and shared between the store
// (zoomImage / setImageViewMode) and the body's pan handler.

export interface ImageViewState {
  /** Multiplicative scale applied to the image. 1 = intrinsic size. */
  scale: number
  /** 'fit' = object-contain to the panel (scale pinned to 1);
   *  'actual' = render at `scale` × intrinsic pixels. */
  mode: 'fit' | 'actual'
}

/** The render-reproducing default for a file with no stored view state. */
export const DEFAULT_IMAGE_VIEW: ImageViewState = { scale: 1, mode: 'fit' }

/** Lower / upper bounds on scale. Below MIN the image is unusably small; above
 *  MAX the browser starts allocating huge backing stores for a scaled bitmap. */
export const MIN_SCALE = 0.1
export const MAX_SCALE = 8

/** Clamp a scale into [MIN_SCALE, MAX_SCALE]. NaN / ≤0 collapse to MIN_SCALE so a
 *  bad value can never produce a 0/NaN transform; +Infinity saturates at MAX. */
export function clampScale(scale: number): number {
  if (Number.isNaN(scale) || scale <= 0) return MIN_SCALE
  // Math.max/min lets +Infinity saturate to MAX_SCALE (a "too big" input).
  return Math.min(MAX_SCALE, Math.max(MIN_SCALE, scale))
}

/** Multiply the current scale by `factor` (e.g. 1.25 to zoom in, 0.8 out) and
 *  clamp. A non-finite/≤0 factor is treated as no-op (returns the clamped
 *  current scale) rather than corrupting state. */
export function zoomStep(scale: number, factor: number): number {
  const base = clampScale(scale)
  if (!Number.isFinite(factor) || factor <= 0) return base
  return clampScale(base * factor)
}

/**
 * Clamp a pan translation to the pannable range for a scaled image inside a
 * container. `overflowX/Y` is how many CSS px the (scaled) content exceeds the
 * container on each axis; the image can be dragged at most half the overflow in
 * either direction (so an edge reaches the container edge, never past it). When
 * the content fits (overflow ≤ 0) the axis is pinned to 0 — nothing to pan.
 */
export function clampTranslate(
  tx: number,
  ty: number,
  overflowX: number,
  overflowY: number,
): { x: number; y: number } {
  const maxX = Math.max(0, overflowX) / 2
  const maxY = Math.max(0, overflowY) / 2
  const clamp1 = (v: number, max: number) =>
    max === 0 || !Number.isFinite(v) ? 0 : Math.min(max, Math.max(-max, v))
  return { x: clamp1(tx, maxX), y: clamp1(ty, maxY) }
}

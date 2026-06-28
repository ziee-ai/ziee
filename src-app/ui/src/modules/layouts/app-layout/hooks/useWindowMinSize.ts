import { type RefObject, useEffect, useRef, useState } from 'react'
import { useWindowSize } from 'react-use'
import { useAppLayoutStore } from '@/modules/layouts/app-layout/AppLayout.store'

// Note: `useEffect` and `useState` are still imported because
// `useMainContentMinSize` below uses them. `useRef` is used by
// the hysteresis tracker in `useWindowMinSize`.

/**
 * Hysteresis buffer (px) for breakpoint flips. Once a breakpoint
 * has crossed in one direction, the viewport must move BEYOND
 * that breakpoint by this much in the opposite direction before
 * it flips back. Without it, slow window drags right at the
 * threshold cause the sidebar's mode to flip-flop many times,
 * which reads as the panel "appearing and disappearing".
 */
const HYSTERESIS_PX = 24

export type Breakpoint =
  | 'xxs'
  | 'xs'
  | 'sm'
  | 'md'
  | 'lg'
  | 'xl'
  | '2xl'
  | '3xl'

const breakpointValues: Record<Breakpoint, number> = {
  xxs: 0,
  xs: 480,
  sm: 640,
  md: 768,
  lg: 1024,
  xl: 1280,
  '2xl': 1536,
  '3xl': 1920,
}

export type MinSize = {
  xxs: boolean
  xs: boolean
  sm: boolean
  md: boolean
  lg: boolean
  xl: boolean
  '2xl': boolean
  '3xl': boolean
}

// Each breakpoint key X is true when the viewport width is AT MOST
// breakpointValues[X] — i.e., `minSize.sm === true` means "viewport
// is at most 640px wide (small phone / small tablet portrait)".
//
// Prior to 2026-05 this table was misaligned: every key compared
// against the next-larger threshold (xs ≤ 640 instead of ≤ 480),
// duplicating xl/2xl at 1280 and flipping `3xl` to `>`. Consumers
// believed they were checking ≤ 480 for mobile and were actually
// catching 640px tablets too. Fixed: each key now uses its own
// threshold consistently with the names above.
const calculateMinSize = (width: number): MinSize => ({
  xxs: width <= breakpointValues.xxs,
  xs: width <= breakpointValues.xs,
  sm: width <= breakpointValues.sm,
  md: width <= breakpointValues.md,
  lg: width <= breakpointValues.lg,
  xl: width <= breakpointValues.xl,
  '2xl': width <= breakpointValues['2xl'],
  '3xl': width <= breakpointValues['3xl'],
})

export const useWindowMinSize = (): MinSize => {
  const { width } = useWindowSize()
  const prevRef = useRef<MinSize | null>(null)

  const next = calculateMinSize(width)
  const resolved = applyHysteresis(width, next, prevRef.current)
  prevRef.current = resolved
  return resolved
}

// Apply the same per-breakpoint hysteresis used by useWindowMinSize.
// Takes the raw width-derived booleans `next` and the previously
// committed booleans `prev`, returns the booleans that should
// actually be exposed (sticky in the buffer band).
function applyHysteresis(
  width: number,
  next: MinSize,
  prev: MinSize | null,
): MinSize {
  if (!prev) return next
  const resolved: MinSize = { ...next }
  ;(Object.keys(breakpointValues) as Breakpoint[]).forEach(bp => {
    const threshold = breakpointValues[bp]
    resolved[bp] = prev[bp]
      ? width <= threshold + HYSTERESIS_PX
      : width <= threshold
  })
  return resolved
}

/**
 * Observe a specific element's width and return MinSize booleans
 * derived from it. Use when a page wants to lay itself out based
 * on ITS OWN container width — not the viewport, not the AppLayout
 * main-content. Pass a ref to the element to observe.
 *
 * Applies the same hysteresis as `useWindowMinSize` so dragging
 * the surrounding container across a threshold doesn't flip-flop
 * the booleans.
 */
export const useElementMinSize = (
  ref: RefObject<HTMLElement | null>,
): MinSize => {
  const [minSize, setMinSize] = useState<MinSize>(() => calculateMinSize(0))
  const prevRef = useRef<MinSize | null>(null)

  useEffect(() => {
    const el = ref.current
    if (!el) return

    const apply = (width: number) => {
      const resolved = applyHysteresis(
        width,
        calculateMinSize(width),
        prevRef.current,
      )
      prevRef.current = resolved
      setMinSize(prev => {
        const isEqual = (Object.keys(resolved) as Breakpoint[]).every(
          k => prev[k] === resolved[k],
        )
        return isEqual ? prev : resolved
      })
    }

    // Seed with the current width before subscribing.
    apply(el.getBoundingClientRect().width)

    const ro = new ResizeObserver(entries => {
      for (const entry of entries) apply(entry.contentRect.width)
    })
    ro.observe(el)
    return () => ro.disconnect()
  }, [ref])

  return minSize
}

export const useMainContentMinSize = (): MinSize => {
  const [minSize, setMinSize] = useState<MinSize>(() => {
    const currentWidth = useAppLayoutStore.getState().mainContentWidth
    return calculateMinSize(currentWidth)
  })

  useEffect(() => {
    const updateMinSize = (state: any) => {
      const width: number = state.mainContentWidth
      setMinSize(prev => {
        const resolved = applyHysteresis(width, calculateMinSize(width), prev)
        const isEqual = (Object.keys(resolved) as Breakpoint[]).every(
          k => prev[k] === resolved[k],
        )
        return isEqual ? prev : resolved
      })
    }

    // Subscribe to future changes
    let unsub = useAppLayoutStore.subscribe(updateMinSize)

    return () => {
      unsub()
    }
  }, [])

  return minSize
}

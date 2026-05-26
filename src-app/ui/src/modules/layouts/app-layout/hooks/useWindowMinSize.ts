import { useEffect, useState } from 'react'
import { useWindowSize } from 'react-use'
import { useAppLayoutStore } from '@/modules/layouts/app-layout/AppLayout.store'

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
  return calculateMinSize(width)
}

export const useMainContentMinSize = (): MinSize => {
  const [minSize, setMinSize] = useState<MinSize>(() => {
    const currentWidth = useAppLayoutStore.getState().mainContentWidth
    return calculateMinSize(currentWidth)
  })

  useEffect(() => {
    const updateMinSize = (state: any) => {
      const newMinSize = calculateMinSize(state.mainContentWidth)

      // Only update if the new minSize is different from the current one
      setMinSize(prevMinSize => {
        const isEqual = Object.keys(newMinSize).every(
          key =>
            prevMinSize[key as keyof MinSize] ===
            newMinSize[key as keyof MinSize],
        )
        return isEqual ? prevMinSize : newMinSize
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

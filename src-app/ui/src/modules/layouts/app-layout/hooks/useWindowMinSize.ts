import { useEffect, useState } from 'react'
import { useAppLayoutStore } from '@/modules/layouts/app-layout/appLayout'

// Shim → @ziee/shell. The pure, store-free breakpoint hooks + helpers moved to
// `@ziee/shell/hooks/useWindowMinSize`. The store-coupled `useMainContentMinSize`
// variant (below) stays app-side — it reads `useAppLayoutStore` directly — and
// composes the moved `calculateMinSize` + `applyHysteresis` helpers.
export {
  useWindowMinSize,
  useElementMinSize,
  calculateMinSize,
  applyHysteresis,
  breakpointValues,
} from '@ziee/shell/hooks/useWindowMinSize'
export type { Breakpoint, MinSize } from '@ziee/shell/hooks/useWindowMinSize'

import {
  calculateMinSize,
  applyHysteresis,
  type Breakpoint,
  type MinSize,
} from '@ziee/shell/hooks/useWindowMinSize'

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

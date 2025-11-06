import { Suspense, useEffect, isValidElement, lazy, useMemo } from 'react'
import { useRouterStore } from '@/core/router/store'
import type { GlobalComponent } from '@/core/router/types'

/**
 * Wrapper component that renders a global component.
 *
 * Components are mounted once and stay mounted. They control their own
 * visibility through props (e.g., drawer open/close animations).
 */
function GlobalComponentWrapper({ comp }: { comp: GlobalComponent }) {
  // Memoize the component rendering to prevent recreating lazy components
  const renderedComponent = useMemo(() => {
    // If it's already a valid React element, return as-is
    if (isValidElement(comp.component)) {
      return comp.component
    }

    // Otherwise, it's a function (preload function from lazyWithPreload), wrap with lazy
    const LazyComponent = lazy(comp.component as () => Promise<{ default: React.ComponentType<any> }>)
    return <LazyComponent />
  }, [comp.component])

  return (
    <Suspense fallback={null}>
      {renderedComponent}
    </Suspense>
  )
}

/**
 * Extracts the preload function from a lazy component.
 * Works with components created by lazyWithPreload().
 */
function getPreloadFunction(component: any): (() => void) | null {
  // If component is already a valid React element, no preload needed
  if (isValidElement(component)) {
    return null
  }

  // If it's a function (from lazyWithPreload), that function IS the preload
  if (typeof component === 'function') {
    return component
  }

  return null
}

/**
 * Hook to handle automatic preloading of global components when browser is idle.
 */
function useGlobalComponentPreloading(components: GlobalComponent[]) {
  useEffect(() => {
    if (components.length === 0) return

    const preloadIdleComponents = () => {
      components.forEach(comp => {
        const preload = getPreloadFunction(comp.component)
        if (preload) {
          // Use requestIdleCallback if available, otherwise setTimeout
          if ('requestIdleCallback' in window) {
            requestIdleCallback(
              () => preload(),
              { timeout: 2000 }  // Fallback after 2s if not idle
            )
          } else {
            setTimeout(preload, 1000)
          }
        }
      })
    }

    // Wait a bit for critical resources to load first
    const timer = setTimeout(preloadIdleComponents, 1000)
    return () => clearTimeout(timer)
  }, [components])
}

/**
 * Renders all registered global components from all modules.
 *
 * Each component:
 * - Is wrapped in Suspense for lazy loading
 * - Stays mounted once loaded (never unmounts)
 * - Manages its own visibility through props (e.g., drawer open/close)
 * - Preloads when browser is idle
 *
 * Mount this component at the app root level.
 */
export function GlobalComponentRenderer() {
  const { globalComponents } = useRouterStore()

  // Handle automatic preloading when idle
  useGlobalComponentPreloading(globalComponents)

  return (
    <>
      {globalComponents.map(comp => (
        <GlobalComponentWrapper key={comp.id} comp={comp} />
      ))}
    </>
  )
}

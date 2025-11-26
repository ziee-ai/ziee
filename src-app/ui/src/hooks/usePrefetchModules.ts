import { useEffect, isValidElement } from 'react'
import { Stores } from '@/core/stores'

/**
 * Hook to prefetch lazy-loaded modules after initial render
 * Uses requestIdleCallback to prefetch when browser is idle
 */
export function usePrefetchModules() {
  const { routes } = Stores.Routes

  useEffect(() => {
    // Check if requestIdleCallback is supported (not available in Safari < 16)
    const prefetch = () => {
      routes.forEach(route => {
        // If element is a function (preload function), call it to trigger the import
        if (
          typeof route.element === 'function' &&
          !isValidElement(route.element)
        ) {
          ;(
            route.element as () => Promise<{
              default: React.ComponentType<any>
            }>
          )()
        }
      })
    }

    if ('requestIdleCallback' in window) {
      const handle = requestIdleCallback(prefetch, { timeout: 2000 })
      return () => cancelIdleCallback(handle)
    } else {
      // Fallback for browsers without requestIdleCallback
      const timer = setTimeout(prefetch, 1000)
      return () => clearTimeout(timer)
    }
  }, [routes])
}

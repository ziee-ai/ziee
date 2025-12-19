import { useMemo, useEffect } from 'react'
import { ThemeProvider } from '@/components/ThemeProvider'
import { loadModules } from '@/modules/loader'
import { setupAccessibilityFixes } from '@/utils/accessibilityFixes'
import { usePrefetchModules } from '@/hooks/usePrefetchModules'
import { Stores } from '@/core/stores'
import { LazyComponentRenderer } from '@/core/components/LazyComponentRenderer'
import type { ComponentRegistration } from '@/core/module-system/types'

// Load all modules before rendering
loadModules()

/**
 * ConditionalComponent - Wrapper that checks shouldMount hook before rendering
 */
function ConditionalComponent({
  registration,
}: {
  registration: ComponentRegistration
}) {
  // Call shouldMount hook if provided, default to true
  const shouldMount = registration.shouldMount?.() ?? true

  // Memoize the component renderer to prevent recreating it when shouldMount changes
  const renderer = useMemo(
    () => (
      <LazyComponentRenderer
        component={registration.component}
        fallback={null}
      />
    ),
    [registration.component],
  )

  if (!shouldMount) {
    return null
  }

  return renderer
}

/**
 * App - Main application component
 *
 * Phase 2: Meta-Framework Architecture
 * - RouterComponent (from router module) handles all routing logic
 * - App.tsx just renders components from modules in order
 * - No routing knowledge here - it's all in RouterComponent
 */
function App() {
  const { components } = Stores.ModuleSystem

  // Setup global accessibility fixes
  useEffect(() => {
    const cleanup = setupAccessibilityFixes()
    return cleanup
  }, [])

  // Prefetch lazy-loaded modules when browser is idle
  usePrefetchModules()

  // Sort components by order (RouterComponent has order: 0, so it renders first)
  const sortedComponents = useMemo(() => {
    return [...components].sort((a, b) => (a.order ?? 0) - (b.order ?? 0))
  }, [components])

  return (
    <ThemeProvider>
      {/* Render components from modules (sorted by order) */}
      {sortedComponents.map(comp => (
        <ConditionalComponent key={comp.id} registration={comp} />
      ))}
    </ThemeProvider>
  )
}

export default App
